// r8s-core/src/repository/variable_repository.rs
//! Interface de repositório para variáveis.

use crate::common::{AsyncResult, EntityId, Result};
use crate::entity::{Variable, VariableScope};
use crate::repository::Repository;
use async_trait::async_trait;

/// Interface para repositório de variáveis
#[async_trait]
pub trait VariableRepository: Repository<Variable> {
    /// Busca variáveis globais
    fn find_global_variables(&self) -> AsyncResult<Vec<Variable>>;

    /// Busca variáveis de um workflow específico
    fn find_by_workflow(&self, workflow_id: EntityId) -> AsyncResult<Vec<Variable>>;

    /// Busca variáveis de um usuário específico
    fn find_by_user(&self, user_id: EntityId) -> AsyncResult<Vec<Variable>>;

    /// Busca variáveis de um ambiente específico
    fn find_by_environment(&self, environment: &str) -> AsyncResult<Vec<Variable>>;

    /// Busca uma variável pelo escopo e chave
    fn find_by_scope_and_key(
        &self,
        scope: &VariableScope,
        key: &str,
    ) -> AsyncResult<Option<Variable>>;

    /// Busca variáveis pela chave (em qualquer escopo)
    fn find_by_key(&self, key: &str) -> AsyncResult<Vec<Variable>>;

    /// Atualiza o valor de uma variável
    fn update_value(&self, variable_id: EntityId, value: String) -> AsyncResult<()>;

    /// Define a proteção de uma variável
    fn set_protected(&self, variable_id: EntityId, protected: bool) -> AsyncResult<()>;

    /// Verifica se um usuário tem acesso a uma variável
    fn has_access(&self, variable_id: EntityId, user_id: EntityId) -> AsyncResult<bool>;
}
