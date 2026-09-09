use std::{os::unix::process::CommandExt, process::Stdio};

use clap::Parser;

use crate::daemon::pid_path;

#[derive(clap::Parser)]
#[command(name = "slowshell")]
#[command(version = concat!(env!("CARGO_PKG_VERSION")))]
#[command(about = "Some shell")]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
  Daemon,
  Start,
  Stop,
  Ipc {
    #[arg(name = "Command")]
    command: String,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
}

pub fn cli() {
  let cli = Cli::parse();

  match cli.command {
    Commands::Daemon => {
      crate::daemon::daemon();
    }
    Commands::Start => {
      let mut cmd = std::process::Command::new(
        std::env::current_exe().expect("Current exe could not be determined"),
      );

      cmd
        .args(["daemon"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

      unsafe {
        cmd.pre_exec(|| {
          libc::setsid();
          Ok(())
        });
      }

      match cmd.spawn() {
        Ok(child) => {
          let path = pid_path().expect("XDG_RUNTIME_DIR not set");
          if let Err(e) = std::fs::write(&path, child.id().to_string()) {
            eprintln!("Failed to save PID file: {}", e);
          } else {
            println!("Started daemon with PID {}", child.id());
          }
        }
        Err(e) => eprintln!("Failed to spawn daemon: {}", e),
      }
    }
    Commands::Stop => {
      let path = pid_path().expect("XDG_RUNTIME_DIR not set");

      let pid_str = match std::fs::read_to_string(&path) {
        Ok(content) => content.trim().to_string(),
        Err(_) => {
          eprintln!("Daemon is not running.");
          return;
        }
      };

      let pid: libc::pid_t = match pid_str.parse() {
        Ok(num) => num,
        Err(_) => {
          eprintln!("Invalid PID found in file.");
          return;
        }
      };

      unsafe {
        if libc::kill(-pid, libc::SIGTERM) == 0 {
          println!("Stopped daemon (PID {}).", pid);
        } else {
          let errno = *libc::__errno_location();
          if errno == libc::ESRCH {
            println!("No daemon. Cleaning up stale PID file.");
          } else {
            eprintln!("Failed to stop daemon. libc errno: {}", errno);
          }
        }
      }

      let _ = std::fs::remove_file(path);
    }
    Commands::Ipc { command, args } => {
      let mut payload = format!("exec {}", command);

      for arg in args {
        if arg.contains(char::is_whitespace) {
          payload.push_str(&format!(" \"{}\"", arg));
        } else {
          payload.push_str(&format!(" {}", arg));
        }
      }

      slowshell_ipc::send(payload).expect("IPC Send failed:");
    }
  }
}
