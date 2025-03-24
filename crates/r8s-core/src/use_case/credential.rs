// r8s-core/src/use_case/credential.rs
//! Casos de uso relacionados a credenciais.

use std::sync::Arc;

use crate::common::{EntityId, Result};
use crate::entity::{Credential, CredentialType};
use crate::error::Error;
use crate::repository::CredentialRepository;

/// Serviço para criptografia de credenciais
pub trait CredentialEncryptionService: Send + Sync {
    /// Criptografa os dados de uma credencial
    fn encrypt(&self, data: serde_json::Value) -> Result<serde_json::Value>;
    
    /// Descriptografa os dados de uma credencial
    fn decrypt(&self, data: serde_json::Value) -> Result<serde_json::Value>;
}

/// Caso de uso para criar uma nova credencial
pub struct CreateCredentialUseCase<R: CredentialRepository, E: CredentialEncryptionService> {
    credential_repository: Arc<R>,
    encryption_service: Arc<E>,
}

/// Entrada para o caso de uso de criação de credencial
pub struct CreateCredentialInput {
    /// Nome da credencial
    pub name: String,
    
    /// Tipo da credencial
    pub type_name: String,
    
    /// Dados da credencial
    pub data: serde_json::Value,
    
    /// ID do proprietário
    pub owner_id: Option<EntityId>,
    
    /// Se a credencial deve ser compartilhada
    pub shared: bool,
    
    /// Notas sobre a credencial
    pub notes: Option<String>,
}

impl<R: CredentialRepository, E: CredentialEncryptionService> CreateCredentialUseCase<R, E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>, encryption_service: Arc<E>) -> Self {
        Self {
            credential_repository,
            encryption_service,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: CreateCredentialInput) -> Result<Credential> {
        // Criptografa os dados da credencial
        let encrypted_data = self.encryption_service.encrypt(input.data)?;
        
        // Cria a credencial
        let mut credential = Credential::new(
            input.name,
            input.type_name,
            encrypted_data,
            input.owner_id,
        );
        
        // Define campos adicionais
        if input.shared {
            credential.shared = true;
        }
        
        credential.notes = input.notes;
        
        // Valida a credencial
        credential.validate()?;
        
        // Salva a credencial
        let credential = self.credential_repository.save(credential).await?;
        
        Ok(credential)
    }
}

/// Caso de uso para obter uma credencial
pub struct GetCredentialUseCase<R: CredentialRepository, E: CredentialEncryptionService> {
    credential_repository: Arc<R>,
    encryption_service: Arc<E>,
}

impl<R: CredentialRepository, E: CredentialEncryptionService> GetCredentialUseCase<R, E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>, encryption_service: Arc<E>) -> Self {
        Self {
            credential_repository,
            encryption_service,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, credential_id: EntityId, user_id: EntityId) -> Result<Credential> {
        // Verifica se o usuário tem acesso à credencial
        let has_access = self.credential_repository.has_access(credential_id, user_id).await?;
        
        if !has_access {
            return Err(Error::Unauthorized(
                format!("Usuário {} não tem acesso à credencial {}", user_id, credential_id)
            ));
        }
        
        // Obtém a credencial
        let mut credential = self.credential_repository.find_by_id(credential_id).await?;
        
        // Descriptografa os dados
        let decrypted_data = self.encryption_service.decrypt(credential.data.clone())?;
        credential.data = decrypted_data;
        
        Ok(credential)
    }
}

/// Caso de uso para compartilhar uma credencial
pub struct ShareCredentialUseCase<R: CredentialRepository> {
    credential_repository: Arc<R>,
}

impl<R: CredentialRepository> ShareCredentialUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>) -> Self {
        Self {
            credential_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, credential_id: EntityId, user_id: EntityId) -> Result<()> {
        // Obtém a credencial
        let credential = self.credential_repository.find_by_id(credential_id).await?;
        
        // Verifica se o usuário é o proprietário
        if let Some(owner_id) = credential.owner_id {
            if owner_id != user_id {
                return Err(Error::Unauthorized(
                    format!("Usuário {} não é proprietário da credencial {}", user_id, credential_id)
                ));
            }
        } else {
            return Err(Error::Unauthorized(
                format!("Credencial {} não tem proprietário definido", credential_id)
            ));
        }
        
        // Compartilha a credencial
        self.credential_repository.share_credential(credential_id).await?;
        
        Ok(())
    }
}

/// Caso de uso para remover o compartilhamento de uma credencial
pub struct UnshareCredentialUseCase<R: CredentialRepository> {
    credential_repository: Arc<R>,
}

impl<R: CredentialRepository> UnshareCredentialUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>) -> Self {
        Self {
            credential_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, credential_id: EntityId, user_id: EntityId) -> Result<()> {
        // Obtém a credencial
        let credential = self.credential_repository.find_by_id(credential_id).await?;
        
        // Verifica se o usuário é o proprietário
        if let Some(owner_id) = credential.owner_id {
            if owner_id != user_id {
                return Err(Error::Unauthorized(
                    format!("Usuário {} não é proprietário da credencial {}", user_id, credential_id)
                ));
            }
        } else {
            return Err(Error::Unauthorized(
                format!("Credencial {} não tem proprietário definido", credential_id)
            ));
        }
        
        // Remove o compartilhamento
        self.credential_repository.unshare_credential(credential_id).await?;
        
        Ok(())
    }
}

/// Caso de uso para atualizar uma credencial
pub struct UpdateCredentialUseCase<R: CredentialRepository, E: CredentialEncryptionService> {
    credential_repository: Arc<R>,
    encryption_service: Arc<E>,
}

/// Entrada para o caso de uso de atualização de credencial
pub struct UpdateCredentialInput {
    /// ID da credencial a ser atualizada
    pub id: EntityId,
    
    /// Novo nome (opcional)
    pub name: Option<String>,
    
    /// Novos dados (opcional)
    pub data: Option<serde_json::Value>,
    
    /// Novas notas (opcional)
    pub notes: Option<String>,
    
    /// ID do usuário que está realizando a atualização
    pub user_id: EntityId,
}

impl<R: CredentialRepository, E: CredentialEncryptionService> UpdateCredentialUseCase<R, E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>, encryption_service: Arc<E>) -> Self {
        Self {
            credential_repository,
            encryption_service,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: UpdateCredentialInput) -> Result<Credential> {
        // Obtém a credencial
        let mut credential = self.credential_repository.find_by_id(input.id).await?;
        
        // Verifica se o usuário tem permissão para atualizar
        if let Some(owner_id) = credential.owner_id {
            if owner_id != input.user_id {
                return Err(Error::Unauthorized(
                    format!("Usuário {} não é proprietário da credencial {}", input.user_id, input.id)
                ));
            }
        } else {
            return Err(Error::Unauthorized(
                format!("Credencial {} não tem proprietário definido", input.id)
            ));
        }
        
        // Atualiza os campos
        if let Some(name) = input.name {
            credential.name = name;
        }
        
        if let Some(notes) = input.notes {
            credential.notes = Some(notes);
        }
        
        // Se novos dados foram fornecidos, criptografa e atualiza
        if let Some(data) = input.data {
            let encrypted_data = self.encryption_service.encrypt(data)?;
            credential.data = encrypted_data;
        }
        
        // Valida a credencial
        credential.validate()?;
        
        // Salva as alterações
        let credential = self.credential_repository.save(credential).await?;
        
        Ok(credential)
    }
}

/// Caso de uso para listar credenciais de um usuário
pub struct ListUserCredentialsUseCase<R: CredentialRepository> {
    credential_repository: Arc<R>,
}

impl<R: CredentialRepository> ListUserCredentialsUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>) -> Self {
        Self {
            credential_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, user_id: EntityId) -> Result<Vec<Credential>> {
        // Busca credenciais do usuário
        let credentials = self.credential_repository.find_by_owner(user_id).await?;
        
        Ok(credentials)
    }
}

/// Caso de uso para listar credenciais compartilhadas
pub struct ListSharedCredentialsUseCase<R: CredentialRepository> {
    credential_repository: Arc<R>,
}

impl<R: CredentialRepository> ListSharedCredentialsUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>) -> Self {
        Self {
            credential_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self) -> Result<Vec<Credential>> {
        // Busca credenciais compartilhadas
        let credentials = self.credential_repository.find_shared_credentials().await?;
        
        Ok(credentials)
    }
}

/// Caso de uso para excluir uma credencial
pub struct DeleteCredentialUseCase<R: CredentialRepository> {
    credential_repository: Arc<R>,
}

impl<R: CredentialRepository> DeleteCredentialUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(credential_repository: Arc<R>) -> Self {
        Self {
            credential_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, credential_id: EntityId, user_id: EntityId) -> Result<()> {
        // Obtém a credencial
        let credential = self.credential_repository.find_by_id(credential_id).await?;
        
        // Verifica se o usuário é o proprietário
        if let Some(owner_id) = credential.owner_id {
            if owner_id != user_id {
                return Err(Error::Unauthorized(
                    format!("Usuário {} não é proprietário da credencial {}", user_id, credential_id)
                ));
            }
        } else {
            return Err(Error::Unauthorized(
                format!("Credencial {} não tem proprietário definido", credential_id)
            ));
        }
        
        // Remove a credencial
        self.credential_repository.delete(credential_id).await?;
        
        Ok(())
    }
}