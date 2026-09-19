use std::{
  io::{BufRead, BufReader, Write},
  os::unix::net::{UnixListener, UnixStream},
  path::PathBuf,
  sync::Arc,
};

use futures_channel::mpsc::UnboundedSender;
use miette::{Context, IntoDiagnostic};
use slowshell_core::{
  listeners::{IpcCommand, ListenerAction},
  message::Message,
  types::{PayloadBuilderRegistry, Ustr},
};

pub struct IpcListener;

impl IpcListener {
  pub fn new(tx: UnboundedSender<Message>, preg: Arc<PayloadBuilderRegistry>) -> Self {
    std::thread::spawn(move || {
      let path = ipc_sock_path();

      println!("IPC socket path: {path:?}");

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
                        let payload = preg.build(&command.command, &command.args);

                        if payload.is_some() || !command.args.is_empty() {
                          let _ = tx.unbounded_send(Message::FdUpdate(ListenerAction::Payload {
                            payload,
                            name: command.command,
                          }));
                        } else {
                          let _ = tx.unbounded_send(Message::FdUpdate(ListenerAction::Named(
                            command.command,
                          )));
                        }
                      }
                      _ => {
                        let _ = tx.unbounded_send(Message::FdUpdate(ListenerAction::Ipc(command)));
                      }
                    }
                  }
                }

                Err(e) => {
                  eprintln!("failed to read from ipc: {e}");
                  break;
                }
              }
            }
          }
        }
      }
    });

    Self
  }

  pub fn parse(content: &str) -> miette::Result<IpcCommand> {
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
      miette::bail!("string quote mismatch");
    }

    let mut iterator = tokens.into_iter();

    let name = iterator
      .next()
      .ok_or_else(|| miette::miette!("missing command name"))?;

    let cmd = iterator
      .next()
      .ok_or_else(|| miette::miette!("missing command"))?;

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

pub fn send(content: String) -> miette::Result<()> {
  let path = ipc_sock_path();

  let mut stream = UnixStream::connect(&path)
    .into_diagnostic()
    .with_context(|| format!("Failed to connect to IPC socket at {:?}", path))?;

  let message = if content.ends_with('\n') {
    content
  } else {
    format!("{}\n", content)
  };

  stream
    .write_all(message.as_bytes())
    .into_diagnostic()
    .context("Failed to write data to IPC socket")?;

  stream
    .flush()
    .into_diagnostic()
    .context("Failed to flush IPC socket stream")?;

  Ok(())
}

pub fn ipc_sock_path() -> PathBuf {
  std::env::var("XDG_RUNTIME_DIR")
    .map(PathBuf::from)
    .map(|p| p.join("slowshell.sock"))
    .unwrap_or(PathBuf::from("/tmp/slowshell.sock"))
}
