use std::{os::unix::process::CommandExt, process::Stdio};

use clap::Parser;
use miette::{Context, IntoDiagnostic};

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
  #[command(about = "Run daemon in foreground")]
  Daemon,
  #[command(about = "Start the daemon in background")]
  Start,
  #[command(about = "Stop the running daemon")]
  Stop,
  #[command(about = "Send an IPC command to the running daemon")]
  Ipc {
    #[arg(name = "Command")]
    command: String,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  #[command(about = "List registered resources")]
  List {
    #[command(subcommand)]
    resource: ListResource,
  },
  #[command(about = "Inspect config")]
  Config {
    #[arg(long, help = "Validate the current config")]
    validate: bool,

    #[arg(long, help = "Show the resolved config")]
    show: bool,

    #[arg(long, help = "Print the path of config in use")]
    current: bool,
  },
}

#[derive(clap::Subcommand)]
enum ListResource {
  #[command(about = "List all styles")]
  Styles,
  #[cfg(feature = "plugins")]
  #[command(about = "List loaded plugins")]
  Plugins,
  #[command(about = "List all renderables")]
  Renderables,
  #[cfg(feature = "panels")]
  #[command(about = "List all components")]
  Components,
  #[command(about = "List all desktop items")]
  Items,
  #[cfg(feature = "spotlight")]
  #[command(about = "List all spotlight modes")]
  Spotlights,
}

pub fn cli() -> miette::Result<()> {
  let cli = Cli::parse();

  match cli.command {
    Commands::Daemon => {
      crate::daemon::daemon()?;
    }
    Commands::Start => {
      let exe = std::env::current_exe()
        .into_diagnostic()
        .wrap_err("Current exe could not be determined")?;

      let mut cmd = std::process::Command::new(exe);

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

      let child = cmd
        .spawn()
        .into_diagnostic()
        .wrap_err("Failed to spawn background daemon")?;

      let path = pid_path().ok_or_else(|| miette::miette!("XDG_RUNTIME_DIR env not set."))?;

      std::fs::write(&path, child.id().to_string())
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write PID file to {:?}", path))?;

      println!("Started daemon with PID {}", child.id());
    }
    Commands::Stop => {
      let path = pid_path().ok_or_else(|| miette::miette!("XDG_RUNTIME_DIR env not set."))?;

      let pid_str = match std::fs::read_to_string(&path) {
        Ok(content) => content.trim().to_string(),
        Err(_) => {
          eprintln!("Daemon is not running.");
          return Ok(());
        }
      };

      let pid: libc::pid_t = pid_str
        .parse()
        .into_diagnostic()
        .wrap_err_with(|| format!("Invalid PID in {:?}", path))?;

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

      slowshell_ipc::send(payload).wrap_err("Failed to send command to slowshell IPC daemon")?;
    }
    Commands::List { resource } => match resource {
      ListResource::Styles => crate::inspect::styles()?,
      #[cfg(feature = "plugins")]
      ListResource::Plugins => crate::inspect::plugins()?,
      ListResource::Renderables => crate::inspect::renderables()?,
      #[cfg(feature = "panels")]
      ListResource::Components => crate::inspect::components()?,
      ListResource::Items => crate::inspect::items()?,
      #[cfg(feature = "spotlight")]
      ListResource::Spotlights => crate::inspect::spotlights()?,
    },
    Commands::Config {
      validate,
      show,
      current,
    } => {
      if validate {
        crate::inspect::config_validate()?;
      }
      if show {
        crate::inspect::config_show()?;
      }
      if current {
        crate::inspect::config_current()?;
      }
      if !validate && !show && !current {
        crate::inspect::config_show()?;
      }
    }
  }

  Ok(())
}
