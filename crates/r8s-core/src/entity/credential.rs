// r8s-core/src/entity/credential.rs
//! Define a entidade Credential, que representa credenciais para serviços externos.

use std::collections::HashMap;
use validator::Validate;

use crate::common::{EntityId, Identifiable, Result, Timestamp, Validatable};
use crate::error::Error;

/// Representa uma credencial para acesso a serviços externos.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct Credential {
    /// Identificador único da credencial
    pub id: EntityId,

    /// Nome da credencial para identificação
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// Tipo da credencial
    pub type_name: String,

    /// Dados da credencial (criptografados ao salvar)
    pub data: serde_json::Value,

    /// ID do usuário proprietário
    pub owner_id: Option<EntityId>,

    /// Se a credencial está compartilhada no sistema
    pub shared: bool,

    /// Notas sobre a credencial
    #[serde(default)]
    pub notes: Option<String>,

    /// Timestamps de criação e atualização
    #[serde(default)]
    pub timestamps: Timestamp,
}

/// Tipos de credenciais suportados
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CredentialType {
    /// Autenticação básica (usuário/senha)
    BasicAuth { username: String, password: String },

    /// Token de autorização
    BearerToken { token: String },

    /// Chave de API
    ApiKey {
        key: String,
        header_name: Option<String>,
    },

    /// Chaves OAuth
    OAuth2 {
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        expiry: Option<chrono::DateTime<chrono::Utc>>,
    },

    /// Certificado X.509
    Certificate {
        cert: String,
        key: String,
        passphrase: Option<String>,
    },

    /// Customizável
    Custom { properties: HashMap<String, String> },
}

impl Credential {
    /// Cria uma nova credencial
    pub fn new(
        name: String,
        type_name: String,
        data: serde_json::Value,
        owner_id: Option<EntityId>,
    ) -> Self {
        Self {
            id: EntityId::new_v4(),
            name,
            type_name,
            data,
            owner_id,
            shared: false,
            notes: None,
            timestamps: Timestamp::new(),
        }
    }

    /// Cria uma nova credencial de autenticação básica
    pub fn new_basic_auth(
        name: String,
        username: String,
        password: String,
        owner_id: Option<EntityId>,
    ) -> Result<Self> {
        let data = serde_json::to_value(CredentialType::BasicAuth { username, password })
            .map_err(|e| Error::Serialization(e.to_string()))?;

        Ok(Self::new(name, "basic_auth".to_string(), data, owner_id))
    }

    /// Cria uma nova credencial de token bearer
    pub fn new_bearer_token(
        name: String,
        token: String,
        owner_id: Option<EntityId>,
    ) -> Result<Self> {
        let data = serde_json::to_value(CredentialType::BearerToken { token })
            .map_err(|e| Error::Serialization(e.to_string()))?;

        Ok(Self::new(name, "bearer_token".to_string(), data, owner_id))
    }

    /// Cria uma nova credencial de chave de API
    pub fn new_api_key(
        name: String,
        key: String,
        header_name: Option<String>,
        owner_id: Option<EntityId>,
    ) -> Result<Self> {
        let data = serde_json::to_value(CredentialType::ApiKey { key, header_name })
            .map_err(|e| Error::Serialization(e.to_string()))?;

        Ok(Self::new(name, "api_key".to_string(), data, owner_id))
    }

    /// Marca a credencial como compartilhada
    pub fn share(&mut self) {
        self.shared = true;
        self.timestamps.update();
    }

    /// Remove o compartilhamento da credencial
    pub fn unshare(&mut self) {
        self.shared = false;
        self.timestamps.update();
    }

    /// Atualiza os dados da credencial
    pub fn update_data(&mut self, data: serde_json::Value) {
        self.data = data;
        self.timestamps.update();
    }

    /// Verifica se um usuário tem acesso à credencial
    pub fn is_accessible_by(&self, user_id: EntityId) -> bool {
        if self.shared {
            return true;
        }

        if let Some(owner_id) = self.owner_id {
            return owner_id == user_id;
        }

        false
    }
}

impl Identifiable for Credential {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Credential {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Validações adicionais específicas
        if self.type_name.is_empty() {
            return Err(Error::Validation(
                "Tipo de credencial não pode ser vazio".to_string(),
            ));
        }

        Ok(())
    }
}
