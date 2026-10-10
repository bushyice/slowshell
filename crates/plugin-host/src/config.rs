use std::collections::HashMap;

use slowshell_config::{
  bool_arg, find_node, float_arg, int_arg, node_children, node_name, str_arg,
};
use slowshell_plugin_abi::{ROption, RResult, RStr};

#[derive(Default)]
pub struct PluginConfigs {
  pub blocks: HashMap<String, HashMap<String, String>>,
}

fn node_value(node: &kdl::KdlNode) -> Option<String> {
  if let Some(value) = str_arg(node, 0) {
    return Some(value.to_owned());
  }
  if let Some(value) = int_arg(node, 0) {
    return Some(value.to_string());
  }
  if let Some(value) = float_arg(node, 0) {
    return Some(value.to_string());
  }
  if let Some(value) = bool_arg(node, 0) {
    return Some(value.to_string());
  }
  None
}

pub(crate) fn parse_plugin_configs(
  nodes: &[kdl::KdlNode],
) -> miette::Result<Option<Box<dyn std::any::Any + Send + Sync>>> {
  let ids = crate::state::plugin_ids().lock().unwrap().clone();
  let mut blocks: HashMap<String, HashMap<String, String>> = HashMap::new();

  for id in ids {
    let Some(node) = find_node(nodes, &id) else {
      continue;
    };

    let mut block = HashMap::new();
    for child in node_children(node) {
      if let Some(value) = node_value(child) {
        block.insert(node_name(child).to_owned(), value);
      }
    }

    blocks.insert(id, block);
  }

  let has_any = !blocks.is_empty();
  *crate::state::plugin_configs().lock().unwrap() = blocks.clone();

  let parsers = std::mem::take(&mut *crate::state::plugin_config_parsers().lock().unwrap());
  for (plugin_id, name, parser) in &parsers {
    let result = match find_node(nodes, name) {
      Some(node) => parser.parse(ROption::RSome(RStr::from_str(&node.to_string()))),
      None => parser.parse(ROption::RNone),
    };

    if let RResult::RErr(error) = result {
      crate::state::record_plugin_error(
        plugin_id,
        1,
        &format!("rejected config entry `{name}`: {error}"),
      );
    }
  }

  {
    let mut guard = crate::state::plugin_config_parsers().lock().unwrap();
    let mut registered_during_parse = std::mem::take(&mut *guard);
    registered_during_parse.extend(parsers);
    *guard = registered_during_parse;
  }

  if has_any {
    Ok(Some(Box::new(PluginConfigs { blocks })))
  } else {
    Ok(None)
  }
}
