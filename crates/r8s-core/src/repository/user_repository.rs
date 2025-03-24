// r8s-core/src/repository/user_repository.rs
//! Interface de repositório para usuários.

use crate::common::{AsyncResult, EntityId, Result};
use crate::entity::{User, UserRole};
use crate::repository::Repository;
use async_trait::async_trait;

/// Interface para repositório de usuários
#[async_trait]
pub trait UserRepository: Repository<User> {
    /// Busca um usuário pelo nome de usuário
    fn find_by_username(&self, username: &str) -> AsyncResult<Option<User>>;

    /// Busca um usuário pelo email
    fn find_by_email(&self, email: &str) -> AsyncResult<Option<User>>;

    /// Busca usuários por papel
    fn find_by_role(&self, role: UserRole) -> AsyncResult<Vec<User>>;

    /// Busca todos os usuários ativos
    fn find_active_users(&self) -> AsyncResult<Vec<User>>;

    /// Atualiza o hash de senha de um usuário
    fn update_password_hash(&self, user_id: EntityId, password_hash: String) -> AsyncResult<()>;

    /// Registra um login de usuário
    fn record_login(&self, user_id: EntityId) -> AsyncResult<()>;

    /// Ativa um usuário
    fn activate_user(&self, user_id: EntityId) -> AsyncResult<()>;

    /// Desativa um usuário
    fn deactivate_user(&self, user_id: EntityId) -> AsyncResult<()>;

    /// Muda o papel de um usuário
    fn change_role(&self, user_id: EntityId, role: UserRole) -> AsyncResult<()>;

    /// Verifica se um nome de usuário já está em uso
    fn username_exists(&self, username: &str) -> AsyncResult<bool>;

    /// Verifica se um email já está em uso
    fn email_exists(&self, email: &str) -> AsyncResult<bool>;
}
