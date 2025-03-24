// r8s-core/src/lib.rs
//! Core domain e lógica de negócios para o orquestrador de workflows r8s.
//!
//! Este crate contém as entidades fundamentais, casos de uso e interfaces
//! de repositório seguindo os princípios de Clean Architecture.

/// Módulo de entidades de domínio
pub mod entity;

/// Módulo de casos de uso
pub mod use_case;

/// Interfaces de repositório
pub mod repository;

/// Interfaces de serviço
pub mod service;

/// Erros do domínio
pub mod error;

/// Tipos e traits comuns
pub mod common;

// Re-exporta tipos comuns para facilitar o uso
pub use common::Result;
pub use entity::*;
pub use error::Error;
