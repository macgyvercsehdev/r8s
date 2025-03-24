// r8s-core/src/use_case/mod.rs
//! Casos de uso que implementam a lógica de negócios da aplicação.
//!
//! Este módulo contém os casos de uso que coordenam as operações
//! entre entidades e repositórios, aplicando as regras de negócio.

pub mod credential;
pub mod execution;
pub mod user;
pub mod variable;
pub mod workflow;

// Re-exportação para facilitar o uso
pub use credential::*;
pub use execution::*;
pub use user::*;
pub use variable::*;
pub use workflow::*;
