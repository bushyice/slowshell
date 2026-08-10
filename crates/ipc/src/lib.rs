use std::{
  io::{BufRead, BufReader},
  os::unix::net::UnixListener,
  path::PathBuf,
};

use anyhow::Context;
use futures_channel::mpsc::UnboundedSender;
use slowshell_core::{
  listeners::{IpcCommand, ListenerAction},
  message::Message,
  types::{ToUstr, Ustr},
};

pub struct IpcListener;

impl IpcListener {
  pub fn new(tx: UnboundedSender<Message>) -> Self {
    std::thread::spawn(move || {
      let path = ipc_sock_path();

      if path.exists() {
        let _ = std::fs::remove_file(&path);
      }

      let listener = UnixListener::bind(path);

      if let Ok(listener) = listener {
        for stream in listener.incoming() {
          if let Ok(stream) = stream {
            let mut reader = BufReader::new(stream);
            loop {
              let mut line = String::new();

              match reader.read_line(&mut line) {
                Ok(0) => break,

                Ok(_) => {
                  if let Ok(command) = Self::parse(&line) {
                    println!("{command:?}");

                    match &*command.name {
                      "exec" => {
                        if command.args.is_empty() {
                          let _ = tx.unbounded_send(Message::FdUpdate(ListenerAction::Named(
                            command.command,
                          )));
                        } else {
                          let _ = tx.unbounded_send(Message::FdUpdate(ListenerAction::Payload {
                            name: command.command,
                            payload: Some(
                              command
                                .args
                                .iter()
                                .map(|x| {
                                  x.split_once("=")
                                    .map(|(n, v)| (n.to_ustr(), v.to_ustr()))
                                    .unwrap_or_else(|| (x.clone(), "_".into()))
                                })
                                .collect(),
                            ),
                          }));
                        }
                      }
                      _ => {
                        let _ = tx.unbounded_send(Message::FdUpdate(ListenerAction::Ipc(command)));
                      }
                    }
                  }
                }

                Err(e) => eprintln!("failed to read from ipc: {e}"),
              }
            }
          }
        }
      }
    });

    Self
  }

  pub fn parse(content: &str) -> anyhow::Result<IpcCommand> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quotes = false;

    for c in content.chars() {
      match c {
        '"' => {
          quotes = !quotes;
        }
        c if c.is_whitespace() && !quotes => {
          if !current.is_empty() {
            tokens.push(current);
            current = String::new();
          }
        }
        _ => {
          current.push(c);
        }
      }
    }

    if !current.is_empty() {
      tokens.push(current);
    }

    if quotes {
      anyhow::bail!("string quote mismatch");
    }

    let mut iterator = tokens.into_iter();

    let name = iterator.next().context("missing command name")?;

    let cmd = iterator.next().context("missing command")?;

    let command = cmd.trim_matches(|c| c == '(' || c == ')').to_string();

    let args: Vec<Ustr> = iterator
      .filter(|s| !s.is_empty())
      .map(|s| s.into())
      .collect();

    Ok(IpcCommand {
      name: name.into(),
      command: command.into(),
      args,
    })
  }
}

pub fn ipc_sock_path() -> PathBuf {
  PathBuf::from("/tmp/slowshell.sock")
}
