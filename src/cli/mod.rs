pub mod init;
pub mod migrate;
pub mod serve;

use clap::{Parser, Subcommand};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "mcpzip", version = VERSION, about = "MCP proxy with search-based tool discovery")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the MCP proxy server
    Serve(serve::ServeArgs),
    /// Interactive setup wizard
    Init,
    /// Migrate from Claude Code config
    Migrate(migrate::MigrateArgs),
}
