# r8s Workflow Orchestrator

r8s is a distributed workflow orchestrator written in Rust, with a focus on performance, reliability, and extensibility. It allows you to create, manage, and execute workflows with a visual editor, supporting various execution environments and integrations through a plugin system.

## Table of Contents

- [Features](#features)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [Configuration](#configuration)
  - [Running the Server](#running-the-server)
- [Project Structure](#project-structure)
- [API Reference](#api-reference)
- [Workflow Concepts](#workflow-concepts)
- [Plugin Development](#plugin-development)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

## Features

- Visual workflow editor
- Distributed execution of workflows
- Multiple execution environments (JavaScript, Lua, WebAssembly)
- Extensible plugin system
- PostgreSQL persistence layer
- RESTful API
- Role-based access control
- Credential management for external services
- Variable management across different scopes

## Getting Started

### Prerequisites

Before you begin, ensure you have the following installed:

- Rust (1.70 or later): [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
- PostgreSQL (14 or later): [https://www.postgresql.org/download/](https://www.postgresql.org/download/)
- Git (optional): [https://git-scm.com/downloads](https://git-scm.com/downloads)

### Installation

1. Clone the repository:

```bash
git clone https://github.com/r8s-team/r8s.git
cd r8s
```

2. Build the project:

```bash
cargo build --release
```

3. Create a database in PostgreSQL:

```bash
createdb r8s
```

### Configuration

r8s uses a combination of configuration files and environment variables for its settings. Create a `.env` file in the project root with the following variables:

```env
# Database Configuration
R8S_DATABASE__URL=postgres://username:password@localhost:5432/r8s
R8S_DATABASE__MAX_CONNECTIONS=5
R8S_DATABASE__RUN_MIGRATIONS=true

# API Configuration
R8S_API__HOST=127.0.0.1
R8S_API__PORT=3000
R8S_API__JWT_SECRET=your_secure_jwt_secret_key_here
R8S_API__ENABLE_OPENAPI=true

# Worker Configuration
R8S_WORKER__THREADS=4
R8S_WORKER__MAX_CONCURRENT_EXECUTIONS=10

# Plugin Configuration
R8S_PLUGIN__LOAD_ON_STARTUP=true
R8S_PLUGIN__PLUGIN_DIRS=./plugins
```

Replace `username:password` with your PostgreSQL credentials.

Alternatively, you can create a `r8s.toml` file with the following content:

```toml
[database]
url = "postgres://username:password@localhost:5432/r8s"
max_connections = 5
run_migrations = true

[api]
host = "127.0.0.1"
port = 3000
jwt_secret = "your_secure_jwt_secret_key_here"
enable_openapi = true

[worker]
threads = 4
max_concurrent_executions = 10

[plugin]
load_on_startup = true
plugin_dirs = ["./plugins"]
```

### Running the Server

1. Initialize the r8s instance:

```bash
cargo run -- init
```

This will guide you through the initial setup, creating necessary database tables and an admin user.

2. Start the server:

```bash
cargo run -- serve
```

The r8s server will start on the configured host and port (default: http://127.0.0.1:3000).

3. Access the web interface:

Open your browser and navigate to http://127.0.0.1:3000 to access the r8s web interface.

4. API Documentation:

If you enabled OpenAPI, you can view the API documentation at http://127.0.0.1:3000/swagger-ui.

## Project Structure

The r8s project is organized into several crates:

- `r8s-core`: Core domain entities and business logic
- `r8s-persistence`: Database abstraction and PostgreSQL implementation
- `r8s-execution`: Workflow execution engine
- `r8s-plugin`: Plugin system for extending functionality
- `r8s-api`: RESTful API layer
- `r8s-cli`: Command-line interface

## API Reference

r8s provides a comprehensive RESTful API for managing workflows, executions, credentials, users, and variables. The main endpoints include:

- `/api/workflows`: Workflow management
- `/api/executions`: Workflow execution
- `/api/credentials`: Credential management
- `/api/users`: User management
- `/api/variables`: Variable management
- `/api/auth`: Authentication

For detailed API documentation, refer to the OpenAPI documentation at http://127.0.0.1:3000/swagger-ui when the server is running.

## Workflow Concepts

A workflow in r8s consists of:

- **Nodes**: Functional units that perform specific operations
- **Connections**: Links between nodes that define the flow of data
- **Credentials**: Secure storage for authentication information
- **Variables**: Named values that can be used across workflows
- **Executions**: Instances of workflow runs with their results

Workflows can be created, edited, and executed through the web interface or API.

## Plugin Development

r8s supports extending its functionality through plugins. To create a plugin:

1. Create a new Rust library crate
2. Implement the `Plugin` trait from `r8s-plugin`
3. Expose your plugin via the `create_plugin` function
4. Build as a dynamic library (`.so`, `.dll`, or `.dylib`)
5. Place the compiled library in one of the configured plugin directories

Example plugin structure:

```rust
use r8s_plugin::{Plugin, PluginMetadata};

struct MyPlugin {
    metadata: PluginMetadata,
}

#[async_trait::async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&self) -> Result<()> {
        // Plugin initialization code
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // Cleanup code
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[no_mangle]
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(MyPlugin {
        metadata: PluginMetadata {
            id: "my-plugin".to_string(),
            name: "My Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Your Name".to_string(),
            description: "Description of my plugin".to_string(),
            min_core_version: "0.1.0".to_string(),
            website: None,
            license: Some("MIT".to_string()),
            icon: None,
        }
    })
}
```

## Troubleshooting

### Database Connection Issues

- Ensure PostgreSQL is running and accessible
- Check that the database URL in your configuration is correct
- Verify that the user has appropriate permissions

### API Connection Issues

- Make sure the API server is running on the configured host and port
- Check firewall settings that might block the connection
- Verify that the JWT secret is properly configured

### Plugin Loading Issues

- Ensure plugin libraries are compiled with the same Rust version as r8s
- Verify that plugin directories exist and are properly configured
- Check that plugins implement the required traits correctly

## Contributing

Contributions to r8s are welcome! Please feel free to submit issues, feature requests, and pull requests.

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Commit your changes: `git commit -am 'Add feature'`
4. Push to the branch: `git push origin feature-name`
5. Submit a pull request

## License

r8s is released under the MIT License. See the LICENSE file for details.

---

For questions, support, or feedback, please open an issue on our GitHub repository or contact the r8s team.
