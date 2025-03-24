// r8s-core/src/repository/credential_repository.rs
//! Interface de repositório para credenciais.

use crate::common::{AsyncResult, EntityId, Result};
use crate::entity::Credential;
use crate::repository::Repository;
use async_trait::async_trait;

/// Interface para repositório de credenciais
#[async_trait]
pub trait CredentialRepository: Repository<Credential> {
    /// Busca credenciais de um usuário específico
    fn find_by_owner(&self, owner_id: EntityId) -> AsyncResult<Vec<Credential>>;

    /// Busca credenciais compartilhadas
    fn find_shared_credentials(&self) -> AsyncResult<Vec<Credential>>;

    /// Busca credenciais pelo tipo
    fn find_by_type(&self, type_name: &str) -> AsyncResult<Vec<Credential>>;

    /// Verifica se um usuário tem acesso a uma credencial
    fn has_access(&self, credential_id: EntityId, user_id: EntityId) -> AsyncResult<bool>;

    /// Obtém credenciais usadas por um nó
    fn find_by_node(&self, node_id: EntityId) -> AsyncResult<Vec<Credential>>;

    /// Atualiza os dados de uma credencial (específico para criptografia)
    fn update_data(
        &self,
        credential_id: EntityId,
        encrypted_data: serde_json::Value,
    ) -> AsyncResult<()>;

    /// Compartilha uma credencial
    fn share_credential(&self, credential_id: EntityId) -> AsyncResult<()>;

    /// Remove o compartilhamento de uma credencial
    fn unshare_credential(&self, credential_id: EntityId) -> AsyncResult<()>;
}
