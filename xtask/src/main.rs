mod html;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "xtask", about = "slowshell development tasks")]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  Page {
    #[arg(value_enum, default_value_t = PageAction::Html)]
    action: PageAction,
    #[arg(long, default_value = "target/page.md")]
    out_md: PathBuf,
    #[arg(long, default_value = "target/page.html")]
    out_html: PathBuf,
  },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum PageAction {
  Build,
  Md,
  Html,
  Check,
}

fn workspace_root() -> PathBuf {
  let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let root = manifest.parent().map(Path::to_path_buf).unwrap_or(manifest);
  root.canonicalize().unwrap_or(root)
}

fn absolute(root: &Path, path: &Path) -> PathBuf {
  if path.is_absolute() {
    path.to_path_buf()
  } else {
    root.join(path)
  }
}

fn display(root: &Path, path: &Path) -> String {
  path
    .strip_prefix(root)
    .map(|path| path.to_string_lossy().replace('\\', "/"))
    .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn run_docs(action: PageAction, root: &Path, out_md: &Path, out_html: &Path) -> Result<()> {
  let assembled = html::assemble(root)?;
  println!("assembled {}", display(root, &root.join("README.md")));

  if action == PageAction::Check {
    return Ok(());
  }

  if matches!(action, PageAction::Build | PageAction::Md) {
    fs::write(out_md, &assembled).with_context(|| format!("writing {}", out_md.display()))?;
    println!("wrote {}", display(root, out_md));
  }

  if matches!(action, PageAction::Build | PageAction::Html) {
    let page = html::render_page(&assembled, "slowshell");
    fs::write(out_html, page).with_context(|| format!("writing {}", out_html.display()))?;
    println!("wrote {}", display(root, out_html));
  }

  Ok(())
}

fn run() -> Result<()> {
  match Cli::parse().command {
    Command::Page {
      action,
      out_md,
      out_html,
    } => {
      let root = workspace_root();
      run_docs(
        action,
        &root,
        &absolute(&root, &out_md),
        &absolute(&root, &out_html),
      )
    }
  }
}

fn main() {
  if let Err(error) = run() {
    eprintln!("error: {error:#}");
    std::process::exit(1);
  }
}
