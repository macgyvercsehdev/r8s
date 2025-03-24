// r8s-core/src/entity/mod.rs
//! Entidades de domínio para o orquestrador de workflows.
//!
//! Estas entidades representam os conceitos fundamentais do sistema
//! e são agnósticas de frameworks ou tecnologias específicas.

pub mod connection;
pub mod credential;
pub mod execution;
pub mod node;
pub mod tag;
pub mod user;
pub mod variable;
pub mod workflow;

// Re-exporta para facilitar o uso
pub use connection::Connection;
pub use credential::Credential;
pub use execution::{Execution, ExecutionLog, ExecutionResult};
pub use node::{Node, NodeType, NodeTypeCategory};
pub use tag::Tag;
pub use user::User;
pub use variable::Variable;
pub use workflow::Workflow;
