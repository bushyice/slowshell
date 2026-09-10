mod app;
mod cli;
mod daemon;
mod eloop;
mod watcher;

fn main() -> miette::Result<()> {
  cli::cli()
}
