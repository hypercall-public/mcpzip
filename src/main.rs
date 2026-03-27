use clap::Parser;
use mcpzip::cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Serve(args) => mcpzip::cli::serve::run_serve(args).await,
        Commands::Init => {
            mcpzip::cli::init::run_init()
        }
        Commands::Migrate(args) => mcpzip::cli::migrate::run_migrate(args),
    };

    if let Err(e) = result {
        eprintln!("mcpzip: error: {}", e);
        std::process::exit(1);
    }
}
