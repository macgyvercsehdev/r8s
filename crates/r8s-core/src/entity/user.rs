// r8s-core/src/entity/user.rs
//! Define a entidade User, que representa um usuário do sistema.

use crate::common::{EntityId, Identifiable, Result, Timestamp, Validatable};
use crate::error::Error;
use validator::Validate;

/// Representa um usuário do sistema.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct User {
    /// Identificador único do usuário
    pub id: EntityId,

    /// Nome de usuário para login
    #[validate(length(min = 3, max = 50))]
    pub username: String,

    /// Email do usuário
    #[validate(email)]
    pub email: String,

    /// Nome completo do usuário
    #[validate(length(min = 1, max = 100))]
    pub full_name: String,

    /// Hash da senha (não armazena senha em texto plano)
    pub password_hash: String,

    /// Papel do usuário no sistema
    pub role: UserRole,

    /// Se o usuário está ativo
    pub active: bool,

    /// Último login do usuário
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,

    /// Configurações de preferências do usuário
    #[serde(default)]
    pub preferences: serde_json::Value,

    /// Timestamps de criação e atualização
    #[serde(default)]
    pub timestamps: Timestamp,
}

/// Papéis de usuário no sistema
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    /// Usuário regular
    User,

    /// Usuário com capacidades de criação avançadas
    PowerUser,

    /// Administrador de time
    TeamAdmin,

    /// Administrador do sistema completo
    Admin,
}

impl User {
    /// Cria um novo usuário (sem hash de senha)
    pub fn new(username: String, email: String, full_name: String, role: UserRole) -> Self {
        Self {
            id: EntityId::new_v4(),
            username,
            email,
            full_name,
            password_hash: String::new(), // Deve ser definido separadamente
            role,
            active: true,
            last_login: None,
            preferences: serde_json::Value::Object(serde_json::Map::new()),
            timestamps: Timestamp::new(),
        }
    }

    /// Registra um login do usuário
    pub fn record_login(&mut self) {
        self.last_login = Some(chrono::Utc::now());
        self.timestamps.update();
    }

    /// Ativa o usuário
    pub fn activate(&mut self) {
        self.active = true;
        self.timestamps.update();
    }

    /// Desativa o usuário
    pub fn deactivate(&mut self) {
        self.active = false;
        self.timestamps.update();
    }

    /// Muda o papel do usuário
    pub fn change_role(&mut self, role: UserRole) {
        self.role = role;
        self.timestamps.update();
    }

    /// Verifica se o usuário tem permissão de administrador
    pub fn is_admin(&self) -> bool {
        self.role == UserRole::Admin
    }

    /// Verifica se o usuário tem permissão de administrador de time
    pub fn is_team_admin(&self) -> bool {
        self.role == UserRole::TeamAdmin || self.role == UserRole::Admin
    }

    /// Define uma preferência do usuário
    pub fn set_preference<T: serde::Serialize>(&mut self, key: &str, value: T) -> Result<()> {
        let value = serde_json::to_value(value).map_err(|e| Error::Serialization(e.to_string()))?;

        if let serde_json::Value::Object(prefs) = &mut self.preferences {
            prefs.insert(key.to_string(), value);
        } else {
            let mut map = serde_json::Map::new();
            map.insert(key.to_string(), value);
            self.preferences = serde_json::Value::Object(map);
        }

        self.timestamps.update();
        Ok(())
    }

    /// Obtém uma preferência do usuário
    pub fn get_preference<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        if let serde_json::Value::Object(prefs) = &self.preferences {
            if let Some(value) = prefs.get(key) {
                let result = serde_json::from_value(value.clone())
                    .map_err(|e| Error::Serialization(e.to_string()))?;
                Ok(Some(result))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

impl Identifiable for User {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for User {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Validações adicionais específicas
        if self.password_hash.is_empty() {
            return Err(Error::Validation("Hash de senha é obrigatório".to_string()));
        }

        Ok(())
    }
}
