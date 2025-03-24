// crates/r8s-cli/src/commands/init.rs
//! Command to initialize a new r8s instance.

use clap::Args;
use console::style;
use dialoguer::{Confirm, Input, Password};
use std::fs;
use std::path::PathBuf;
use tracing::info;

use crate::config::Config;
use r8s_core::common::EntityId;
use r8s_core::entity::{User, UserRole};
use r8s_persistence::connection::ConnectionManager;

/// Arguments for the init command.
#[derive(Args)]
pub struct InitArgs {
    /// Output configuration file
    #[arg(short, long, default_value = "r8s.toml")]
    config_file: PathBuf,

    /// Skip interactive prompts
    #[arg(long)]
    non_interactive: bool,

    /// Database connection string
    #[arg(long)]
    db_url: Option<String>,

    /// API host
    #[arg(long)]
    api_host: Option<String>,

    /// API port
    #[arg(long)]
    api_port: Option<u16>,

    /// Admin username
    #[arg(long)]
    admin_username: Option<String>,

    /// Admin email
    #[arg(long)]
    admin_email: Option<String>,

    /// Admin password
    #[arg(long)]
    admin_password: Option<String>,
}

/// Execute the init command.
pub async fn execute(args: InitArgs, mut config: Config) -> anyhow::Result<()> {
    println!("{}", style("r8s Initialization").bold().underlined());
    println!("This will set up a new r8s instance.");

    // Configure database
    if !args.non_interactive {
        config.database.url = Input::<String>::new()
            .with_prompt("Database connection URL")
            .default(config.database.url)
            .interact()?;
    } else if let Some(db_url) = args.db_url {
        config.database.url = db_url;
    }

    // Configure API server
    if !args.non_interactive {
        let host = Input::<String>::new()
            .with_prompt("API server host")
            .default(config.api.host.to_string())
            .interact()?;
        config.api.host = host.parse()?;

        config.api.port = Input::<u16>::new()
            .with_prompt("API server port")
            .default(config.api.port)
            .interact()?;

        // Generate a random JWT secret
        let use_random_secret = Confirm::new()
            .with_prompt("Generate a random JWT secret key?")
            .default(true)
            .interact()?;

        if use_random_secret {
            config.api.jwt_secret = uuid::Uuid::new_v4().to_string();
        } else {
            config.api.jwt_secret = Input::<String>::new()
                .with_prompt("JWT secret key")
                .default(config.api.jwt_secret)
                .interact()?;
        }
    } else {
        if let Some(api_host) = args.api_host {
            config.api.host = api_host.parse()?;
        }

        if let Some(api_port) = args.api_port {
            config.api.port = api_port;
        }
    }

    // Save configuration
    let config_toml = toml::to_string_pretty(&config)?;
    fs::write(&args.config_file, config_toml)?;

    println!(
        "{} Configuration saved to {:?}",
        style("✓").green(),
        args.config_file
    );

    // Set up database
    println!("Connecting to database and setting up schema...");
    let conn_manager =
        ConnectionManager::new(&config.database.url, config.database.max_connections)?;

    conn_manager.run_migrations().await?;
    println!("{} Database schema initialized", style("✓").green());

    // Create admin user
    if !args.non_interactive {
        let create_admin = Confirm::new()
            .with_prompt("Create an admin user?")
            .default(true)
            .interact()?;

        if create_admin {
            create_admin_user(&conn_manager, None, None, None).await?;
        }
    } else if args.admin_username.is_some()
        || args.admin_email.is_some()
        || args.admin_password.is_some()
    {
        create_admin_user(
            &conn_manager,
            args.admin_username.as_deref(),
            args.admin_email.as_deref(),
            args.admin_password.as_deref(),
        )
        .await?;
    }

    println!("{} r8s initialization complete!", style("✓").green());
    println!();
    println!("To start the server, run:");
    println!("  r8s serve");

    Ok(())
}

/// Create an admin user in the database.
async fn create_admin_user(
    conn_manager: &ConnectionManager,
    username: Option<&str>,
    email: Option<&str>,
    password: Option<&str>,
) -> anyhow::Result<()> {
    // Prompt for admin user details if not provided
    let username = match username {
        Some(u) => u.to_string(),
        None => Input::<String>::new()
            .with_prompt("Admin username")
            .default("admin".to_string())
            .interact()?,
    };

    let email = match email {
        Some(e) => e.to_string(),
        None => Input::<String>::new()
            .with_prompt("Admin email")
            .interact()?,
    };

    let password = match password {
        Some(p) => p.to_string(),
        None => Password::new()
            .with_prompt("Admin password")
            .with_confirmation("Confirm password", "Passwords don't match")
            .interact()?,
    };

    // Create user in database
    println!("Creating admin user...");

    // Create a connection to the database
    let conn = conn_manager.get_connection().await?;

    // Hash the password (simple hash for example, use a proper algorithm in production)
    let password_hash = format!("hashed_{}", password);

    // Check if user already exists
    let existing_user = sqlx::query("SELECT 1 FROM users WHERE username = $1 OR email = $2")
        .bind(&username)
        .bind(&email)
        .fetch_optional(&conn)
        .await?;

    if existing_user.is_some() {
        println!("{} Admin user already exists", style("!").yellow());
        return Ok(());
    }

    // Create the user
    let user_id = EntityId::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        "INSERT INTO users 
         (id, username, email, full_name, password_hash, role, active, 
         preferences, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(Uuid::from_bytes(user_id.as_bytes()))
    .bind(&username)
    .bind(&email)
    .bind(&format!("{} (Admin)", username))
    .bind(&password_hash)
    .bind("admin")
    .bind(true)
    .bind(serde_json::Value::Object(serde_json::Map::new()))
    .bind(now)
    .bind(now)
    .execute(&conn)
    .await?;

    println!("{} Admin user created successfully", style("✓").green());

    Ok(())
}
