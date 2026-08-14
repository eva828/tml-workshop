mod cli;

use std::path::PathBuf;

use clap::Parser;
use tml_workshop::CdnCache;

use cli::args::{Cli, Command};

fn main() {
    let args = Cli::parse();
    cli::init_tracing(args.verbose);

    let cache = resolve_cache(args.no_cache);

    let runtime = tokio::runtime::Runtime::new().expect("failed to initialize tokio runtime");
    let result = runtime.block_on(run(args.command, cache));
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn resolve_cache(no_cache: bool) -> Option<CdnCache> {
    if no_cache {
        return None;
    }
    let dir = std::env::var("TML_WORKSHOP_CACHE_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|b| b.cache_dir().join("tml-workshop")))?;
    Some(CdnCache::new(dir))
}

async fn run(command: Command, cache: Option<CdnCache>) -> Result<(), cli::CliError> {
    match command {
        Command::Info(args) => cli::info::run(args, cache).await,
        Command::Versions(args) => cli::versions::run(args, cache).await,
        Command::Download(args) => cli::download::run(args, cache).await,
        Command::Localize(args) => cli::localize::run(args, cache).await,
    }
}
