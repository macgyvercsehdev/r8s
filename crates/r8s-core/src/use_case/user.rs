// r8s-core/src/use_case/user.rs
//! Casos de uso relacionados a usuários.

use std::sync::Arc;

use crate::common::{EntityId, Result};
use crate::entity::{User, UserRole};
use crate::error::Error;
use crate::repository::UserRepository;

/// Serviço para hashing de senhas
pub trait PasswordHashService: Send + Sync {
    /// Gera um hash para uma senha
    fn hash_password(&self, password: &str) -> Result<String>;
    
    /// Verifica se uma senha corresponde a um hash
    fn verify_password(&self, password: &str, hash: &str) -> Result<bool>;
}

/// Caso de uso para registrar um novo usuário
pub struct RegisterUserUseCase<R: UserRepository, P: PasswordHashService> {
    user_repository: Arc<R>,
    password_service: Arc<P>,
}

/// Entrada para o caso de uso de registro de usuário
pub struct RegisterUserInput {
    /// Nome de usuário
    pub username: String,
    
    /// Email
    pub email: String,
    
    /// Nome completo
    pub full_name: String,
    
    /// Senha (em texto plano)
    pub password: String,
    
    /// Papel do usuário
    pub role: Option<UserRole>,
}

impl<R: UserRepository, P: PasswordHashService> RegisterUserUseCase<R, P> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>, password_service: Arc<P>) -> Self {
        Self {
            user_repository,
            password_service,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: RegisterUserInput) -> Result<User> {
        // Verifica se o nome de usuário já está em uso
        if self.user_repository.username_exists(&input.username).await? {
            return Err(Error::Conflict(format!("Nome de usuário '{}' já está em uso", input.username)));
        }
        
        // Verifica se o email já está em uso
        if self.user_repository.email_exists(&input.email).await? {
            return Err(Error::Conflict(format!("Email '{}' já está em uso", input.email)));
        }
        
        // Cria o hash da senha
        let password_hash = self.password_service.hash_password(&input.password)?;
        
        // Cria o usuário
        let mut user = User::new(
            input.username,
            input.email,
            input.full_name,
            input.role.unwrap_or(UserRole::User),
        );
        
        // Define o hash da senha
        user.password_hash = password_hash;
        
        // Valida o usuário
        user.validate()?;
        
        // Salva o usuário
        let user = self.user_repository.save(user).await?;
        
        Ok(user)
    }
}

/// Caso de uso para autenticar um usuário
pub struct AuthenticateUserUseCase<R: UserRepository, P: PasswordHashService> {
    user_repository: Arc<R>,
    password_service: Arc<P>,
}

/// Entrada para o caso de uso de autenticação
pub struct AuthenticateUserInput {
    /// Nome de usuário ou email
    pub username_or_email: String,
    
    /// Senha (em texto plano)
    pub password: String,
}

/// Resultado da autenticação
pub struct AuthenticateUserResult {
    /// Usuário autenticado
    pub user: User,
    
    /// Token de acesso (se implementado)
    pub access_token: Option<String>,
}

impl<R: UserRepository, P: PasswordHashService> AuthenticateUserUseCase<R, P> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>, password_service: Arc<P>) -> Self {
        Self {
            user_repository,
            password_service,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: AuthenticateUserInput) -> Result<AuthenticateUserResult> {
        // Busca o usuário pelo nome de usuário ou email
        let user = if input.username_or_email.contains('@') {
            self.user_repository.find_by_email(&input.username_or_email).await?
        } else {
            self.user_repository.find_by_username(&input.username_or_email).await?
        };
        
        // Verifica se o usuário existe
        let user = match user {
            Some(user) => user,
            None => return Err(Error::Unauthorized("Credenciais inválidas".to_string())),
        };
        
        // Verifica se o usuário está ativo
        if !user.active {
            return Err(Error::Unauthorized("Usuário desativado".to_string()));
        }
        
        // Verifica a senha
        let password_matches = self.password_service.verify_password(&input.password, &user.password_hash)?;
        
        if !password_matches {
            return Err(Error::Unauthorized("Credenciais inválidas".to_string()));
        }
        
        // Registra o login
        self.user_repository.record_login(user.id).await?;
        
        // Obtém o usuário atualizado
        let updated_user = self.user_repository.find_by_id(user.id).await?;
        
        // Retorna o resultado
        Ok(AuthenticateUserResult {
            user: updated_user,
            access_token: None, // Implementação de token seria adicionada aqui
        })
    }
}

/// Caso de uso para atualizar um usuário
pub struct UpdateUserUseCase<R: UserRepository> {
    user_repository: Arc<R>,
}

/// Entrada para o caso de uso de atualização de usuário
pub struct UpdateUserInput {
    /// ID do usuário a ser atualizado
    pub id: EntityId,
    
    /// Novo email (opcional)
    pub email: Option<String>,
    
    /// Novo nome completo (opcional)
    pub full_name: Option<String>,
    
    /// Novas preferências (opcional)
    pub preferences: Option<serde_json::Value>,
}

impl<R: UserRepository> UpdateUserUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>) -> Self {
        Self {
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: UpdateUserInput) -> Result<User> {
        // Busca o usuário
        let mut user = self.user_repository.find_by_id(input.id).await?;
        
        // Atualiza os campos
        if let Some(email) = input.email {
            // Verifica se o email já está em uso por outro usuário
            if self.user_repository.email_exists(&email).await? {
                let existing_user = self.user_repository.find_by_email(&email).await?;
                if let Some(existing_user) = existing_user {
                    if existing_user.id != user.id {
                        return Err(Error::Conflict(format!("Email '{}' já está em uso", email)));
                    }
                }
            }
            
            user.email = email;
        }
        
        if let Some(full_name) = input.full_name {
            user.full_name = full_name;
        }
        
        if let Some(preferences) = input.preferences {
            user.preferences = preferences;
        }
        
        // Valida o usuário
        user.validate()?;
        
        // Salva as alterações
        let user = self.user_repository.save(user).await?;
        
        Ok(user)
    }
}

/// Caso de uso para alterar a senha de um usuário
pub struct ChangePasswordUseCase<R: UserRepository, P: PasswordHashService> {
    user_repository: Arc<R>,
    password_service: Arc<P>,
}

/// Entrada para o caso de uso de alteração de senha
pub struct ChangePasswordInput {
    /// ID do usuário
    pub user_id: EntityId,
    
    /// Senha atual
    pub current_password: String,
    
    /// Nova senha
    pub new_password: String,
}

impl<R: UserRepository, P: PasswordHashService> ChangePasswordUseCase<R, P> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>, password_service: Arc<P>) -> Self {
        Self {
            user_repository,
            password_service,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: ChangePasswordInput) -> Result<()> {
        // Busca o usuário
        let user = self.user_repository.find_by_id(input.user_id).await?;
        
        // Verifica a senha atual
        let password_matches = self.password_service.verify_password(&input.current_password, &user.password_hash)?;
        
        if !password_matches {
            return Err(Error::Unauthorized("Senha atual incorreta".to_string()));
        }
        
        // Gera o hash da nova senha
        let new_password_hash = self.password_service.hash_password(&input.new_password)?;
        
        // Atualiza a senha
        self.user_repository.update_password_hash(input.user_id, new_password_hash).await?;
        
        Ok(())
    }
}

/// Caso de uso para mudar o papel de um usuário (admin)
pub struct ChangeUserRoleUseCase<R: UserRepository> {
    user_repository: Arc<R>,
}

/// Entrada para o caso de uso de mudança de papel
pub struct ChangeUserRoleInput {
    /// ID do usuário a ser alterado
    pub user_id: EntityId,
    
    /// Novo papel
    pub new_role: UserRole,
    
    /// ID do administrador que está realizando a alteração
    pub admin_id: EntityId,
}

impl<R: UserRepository> ChangeUserRoleUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>) -> Self {
        Self {
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: ChangeUserRoleInput) -> Result<User> {
        // Busca o administrador
        let admin = self.user_repository.find_by_id(input.admin_id).await?;
        
        // Verifica se é um administrador
        if !admin.is_admin() {
            return Err(Error::Unauthorized("Apenas administradores podem alterar papéis de usuários".to_string()));
        }
        
        // Busca o usuário
        let user = self.user_repository.find_by_id(input.user_id).await?;
        
        // Verifica se um usuário não está tentando remover o próprio papel de administrador
        if admin.id == user.id && user.is_admin() && input.new_role != UserRole::Admin {
            return Err(Error::Unauthorized("Administradores não podem remover seu próprio papel de administrador".to_string()));
        }
        
        // Muda o papel
        self.user_repository.change_role(input.user_id, input.new_role).await?;
        
        // Obtém o usuário atualizado
        let updated_user = self.user_repository.find_by_id(input.user_id).await?;
        
        Ok(updated_user)
    }
}

/// Caso de uso para ativar um usuário (admin)
pub struct ActivateUserUseCase<R: UserRepository> {
    user_repository: Arc<R>,
}

impl<R: UserRepository> ActivateUserUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>) -> Self {
        Self {
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, user_id: EntityId, admin_id: EntityId) -> Result<User> {
        // Busca o administrador
        let admin = self.user_repository.find_by_id(admin_id).await?;
        
        // Verifica se é um administrador
        if !admin.is_admin() {
            return Err(Error::Unauthorized("Apenas administradores podem ativar usuários".to_string()));
        }
        
        // Ativa o usuário
        self.user_repository.activate_user(user_id).await?;
        
        // Obtém o usuário atualizado
        let updated_user = self.user_repository.find_by_id(user_id).await?;
        
        Ok(updated_user)
    }
}

/// Caso de uso para desativar um usuário (admin)
pub struct DeactivateUserUseCase<R: UserRepository> {
    user_repository: Arc<R>,
}

impl<R: UserRepository> DeactivateUserUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>) -> Self {
        Self {
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, user_id: EntityId, admin_id: EntityId) -> Result<User> {
        // Busca o administrador
        let admin = self.user_repository.find_by_id(admin_id).await?;
        
        // Verifica se é um administrador
        if !admin.is_admin() {
            return Err(Error::Unauthorized("Apenas administradores podem desativar usuários".to_string()));
        }
        
        // Verifica se não está tentando desativar a si mesmo
        if admin.id == user_id {
            return Err(Error::Unauthorized("Administradores não podem desativar sua própria conta".to_string()));
        }
        
        // Desativa o usuário
        self.user_repository.deactivate_user(user_id).await?;
        
        // Obtém o usuário atualizado
        let updated_user = self.user_repository.find_by_id(user_id).await?;
        
        Ok(updated_user)
    }
}

/// Caso de uso para listar usuários (admin)
pub struct ListUsersUseCase<R: UserRepository> {
    user_repository: Arc<R>,
}

impl<R: UserRepository> ListUsersUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(user_repository: Arc<R>) -> Self {
        Self {
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, admin_id: EntityId) -> Result<Vec<User>> {
        // Busca o administrador
        let admin = self.user_repository.find_by_id(admin_id).await?;
        
        // Verifica se é um administrador
        if !admin.is_admin() {
            return Err(Error::Unauthorized("Apenas administradores podem listar todos os usuários".to_string()));
        }
        
        // Busca os usuários ativos
        let users = self.user_repository.find_active_users().await?;
        
        Ok(users)
    }
}