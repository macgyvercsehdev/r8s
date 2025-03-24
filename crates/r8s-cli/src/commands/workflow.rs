// crates/r8s-cli/src/commands/workflow.rs
//! Commands for managing workflows.

use clap::{Args, Subcommand};
use console::style;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};

use r8s_core::common::EntityId;
use r8s_core::entity::{Tag, TagColor, Workflow};
use r8s_persistence::connection::ConnectionManager;
use r8s_persistence::postgres::PostgresWorkflowRepository;

use crate::config::Config;

/// Arguments for the workflow command.
#[derive(Args)]
pub struct WorkflowArgs {
    /// Subcommand to run
    #[command(subcommand)]
    command: WorkflowCommands,
}

/// Available workflow subcommands.
#[derive(Subcommand)]
enum WorkflowCommands {
    /// List all workflows
    List(ListArgs),

    /// Create a new workflow
    Create(CreateArgs),

    /// Import a workflow from JSON
    Import(ImportArgs),

    /// Export a workflow to JSON
    Export(ExportArgs),

    /// Activate a workflow
    Activate(ActivateArgs),

    /// Deactivate a workflow
    Deactivate(DeactivateArgs),
}

/// Arguments for listing workflows.
#[derive(Args)]
struct ListArgs {
    /// Filter by tag
    #[arg(long)]
    tag: Option<String>,

    /// Show only active workflows
    #[arg(long)]
    active: bool,

    /// Show only inactive workflows
    #[arg(long)]
    inactive: bool,
}

/// Arguments for creating a workflow.
#[derive(Args)]
struct CreateArgs {
    /// Workflow name
    #[arg(short, long)]
    name: String,

    /// Workflow description
    #[arg(short, long)]
    description: Option<String>,

    /// Workflow tags (comma-separated)
    #[arg(short, long)]
    tags: Option<String>,
}

/// Arguments for importing a workflow.
#[derive(Args)]
struct ImportArgs {
    /// JSON file containing the workflow
    #[arg(short, long)]
    file: PathBuf,
}

/// Arguments for exporting a workflow.
#[derive(Args)]
struct ExportArgs {
    /// Workflow ID to export
    #[arg(short, long)]
    id: String,

    /// Output file (defaults to workflow-{id}.json)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

/// Arguments for activating a workflow.
#[derive(Args)]
struct ActivateArgs {
    /// Workflow ID to activate
    #[arg(short, long)]
    id: String,
}

/// Arguments for deactivating a workflow.
#[derive(Args)]
struct DeactivateArgs {
    /// Workflow ID to deactivate
    #[arg(short, long)]
    id: String,
}

/// Execute the workflow command.
pub async fn execute(args: WorkflowArgs, config: Config) -> anyhow::Result<()> {
    // Set up database connection
    let conn_manager =
        ConnectionManager::new(&config.database.url, config.database.max_connections)?;

    // Create workflow repository
    let workflow_repo = PostgresWorkflowRepository::new(Arc::new(conn_manager));

    // Execute the appropriate subcommand
    match args.command {
        WorkflowCommands::List(args) => {
            list_workflows(&workflow_repo, args).await?;
        }
        WorkflowCommands::Create(args) => {
            create_workflow(&workflow_repo, args).await?;
        }
        WorkflowCommands::Import(args) => {
            import_workflow(&workflow_repo, args).await?;
        }
        WorkflowCommands::Export(args) => {
            export_workflow(&workflow_repo, args).await?;
        }
        WorkflowCommands::Activate(args) => {
            activate_workflow(&workflow_repo, args).await?;
        }
        WorkflowCommands::Deactivate(args) => {
            deactivate_workflow(&workflow_repo, args).await?;
        }
    }

    Ok(())
}

/// List workflows with optional filtering.
async fn list_workflows(
    workflow_repo: &PostgresWorkflowRepository,
    args: ListArgs,
) -> anyhow::Result<()> {
    // Create filter based on arguments
    let filter = r8s_core::repository::WorkflowFilter {
        name: None,
        tags: args
            .tag
            .map(|tag| vec![EntityId::parse_str(&tag).unwrap_or_else(|_| EntityId::nil())]),
        active: if args.active {
            Some(true)
        } else if args.inactive {
            Some(false)
        } else {
            None
        },
        created_by: None,
        sort_by: Some(r8s_core::repository::WorkflowSortField::Name),
        ascending: true,
        limit: None,
        offset: None,
    };

    // Fetch workflows
    let workflows = workflow_repo.find_by_filter(filter).await?;

    if workflows.is_empty() {
        println!("No workflows found.");
        return Ok(());
    }

    // Display workflows in a table
    println!("Found {} workflows:", workflows.len());
    println!();
    println!(
        "{:<36} {:<30} {:<10} {:<20}",
        "ID", "NAME", "VERSION", "STATUS"
    );
    println!("{}", "-".repeat(100));

    for workflow in workflows {
        println!(
            "{:<36} {:<30} {:<10} {:<20}",
            workflow.id,
            workflow.name,
            workflow.version,
            if workflow.active {
                style("ACTIVE").green()
            } else {
                style("INACTIVE").yellow()
            }
        );
    }

    Ok(())
}

/// Create a new workflow.
async fn create_workflow(
    workflow_repo: &PostgresWorkflowRepository,
    args: CreateArgs,
) -> anyhow::Result<()> {
    // Create workflow entity
    let mut workflow = Workflow::new(args.name, args.description);

    // Save workflow
    workflow = workflow_repo.save(workflow).await?;

    // Add tags if provided
    if let Some(tags_str) = args.tags {
        let tag_names: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();

        for tag_name in tag_names {
            let tag = Tag::new(tag_name.to_string(), TagColor::default(), None, None);

            workflow_repo.add_tag(workflow.id, &tag).await?;
        }
    }

    println!(
        "{} Workflow created with ID: {}",
        style("✓").green(),
        workflow.id
    );

    Ok(())
}

/// Import a workflow from a JSON file.
async fn import_workflow(
    workflow_repo: &PostgresWorkflowRepository,
    args: ImportArgs,
) -> anyhow::Result<()> {
    // Read the file
    let json = fs::read_to_string(args.file)?;

    // Parse JSON into workflow
    let mut workflow: Workflow = serde_json::from_str(&json)?;

    // Generate new IDs for the workflow and all its components
    workflow.id = EntityId::new_v4();
    workflow.version = 1;

    // Generate new IDs for nodes
    let old_to_new_node_ids = workflow
        .nodes
        .iter()
        .map(|node| {
            let old_id = node.id;
            (old_id, EntityId::new_v4())
        })
        .collect::<std::collections::HashMap<_, _>>();

    for node in &mut workflow.nodes {
        node.id = old_to_new_node_ids[&node.id];
    }

    // Update connections to use new node IDs
    for connection in &mut workflow.connections {
        connection.id = EntityId::new_v4();
        connection.source_node = old_to_new_node_ids[&connection.source_node];
        connection.target_node = old_to_new_node_ids[&connection.target_node];
    }

    // Save the workflow
    workflow = workflow_repo.save(workflow).await?;

    println!(
        "{} Workflow imported with ID: {}",
        style("✓").green(),
        workflow.id
    );

    Ok(())
}

/// Export a workflow to a JSON file.
async fn export_workflow(
    workflow_repo: &PostgresWorkflowRepository,
    args: ExportArgs,
) -> anyhow::Result<()> {
    // Parse workflow ID
    let workflow_id =
        EntityId::parse_str(&args.id).map_err(|_| anyhow::anyhow!("Invalid workflow ID"))?;

    // Fetch the workflow
    let workflow = workflow_repo.find_by_id(workflow_id).await?;

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&workflow)?;

    // Determine output path
    let output = args
        .output
        .unwrap_or_else(|| PathBuf::from(format!("workflow-{}.json", args.id)));

    // Write to file
    fs::write(&output, json)?;

    println!("{} Workflow exported to {:?}", style("✓").green(), output);

    Ok(())
}

/// Activate a workflow.
async fn activate_workflow(
    workflow_repo: &PostgresWorkflowRepository,
    args: ActivateArgs,
) -> anyhow::Result<()> {
    // Parse workflow ID
    let workflow_id =
        EntityId::parse_str(&args.id).map_err(|_| anyhow::anyhow!("Invalid workflow ID"))?;

    // Fetch the workflow
    let mut workflow = workflow_repo.find_by_id(workflow_id).await?;

    // Activate the workflow
    workflow.active = true;

    // Save the changes
    workflow_repo.save(workflow).await?;

    println!("{} Workflow activated", style("✓").green());

    Ok(())
}

/// Deactivate a workflow.
async fn deactivate_workflow(
    workflow_repo: &PostgresWorkflowRepository,
    args: DeactivateArgs,
) -> anyhow::Result<()> {
    // Parse workflow ID
    let workflow_id =
        EntityId::parse_str(&args.id).map_err(|_| anyhow::anyhow!("Invalid workflow ID"))?;

    // Fetch the workflow
    let mut workflow = workflow_repo.find_by_id(workflow_id).await?;

    // Deactivate the workflow
    workflow.active = false;

    // Save the changes
    workflow_repo.save(workflow).await?;

    println!("{} Workflow deactivated", style("✓").green());

    Ok(())
}
