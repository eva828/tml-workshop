mod cli;

use clap::Parser;

use cli::args::{Cli, Command};

fn main() {
    let args = Cli::parse();
    cli::init_tracing(args.verbose);

    let runtime = tokio::runtime::Runtime::new().expect("failed to initialize tokio runtime");
    let result = runtime.block_on(run(args.command));
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run(command: Command) -> Result<(), cli::CliError> {
    match command {
        Command::Info(args) => cli::info::run(args).await,
        Command::Versions(args) => cli::versions::run(args).await,
        Command::Download(args) => cli::download::run(args).await,
        Command::Localize(args) => cli::localize::run(args).await,
    }
}
