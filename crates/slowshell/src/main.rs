mod app;
mod cli;
mod daemon;
mod eloop;
mod inspect;
mod watcher;

fn main() -> miette::Result<()> {
  cli::cli()
}
