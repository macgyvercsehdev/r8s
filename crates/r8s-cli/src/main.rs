//! Command line interface for the r8s workflow orchestrator.

use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::{error, info};

mod commands;
mod config;

use commands::{init, serve};
use config::Config;

/// r8s workflow orchestrator CLI.
#[derive(Parser)]
#[command(name = "r8s")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Subcommand to run
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new r8s instance
    Init(init::InitArgs),

    /// Start the r8s server
    Serve(serve::ServeArgs),

    /// Manage workflows
    Workflow(workflow::WorkflowArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load environment variables
    dotenv::dotenv().ok();

    // Parse command line arguments
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load()?;

    // Print banner
    print_banner();

    // Execute the appropriate command
    match cli.command {
        Commands::Init(args) => {
            init::execute(args, config).await?;
        }
        Commands::Serve(args) => {
            serve::execute(args, config).await?;
        }
    }

    Ok(())
}

/// Print the r8s banner.
fn print_banner() {
    println!();
    println!("    {}", r"   _____       ".bright_blue());
    println!("    {}", r"  |  __ \      ".bright_blue());
    println!("    {}", r"  | |__) |__   ___ ".bright_blue());
    println!("    {}", r"  |  _  // _| / __|".bright_blue());
    println!("    {}", r"  | | \ \ (_| \__ \".bright_blue());
    println!("    {}", r"  |_|  \_\__,_|___/".bright_blue());
    println!("    {}", "Workflow Orchestrator".bright_blue());
    println!();
}
