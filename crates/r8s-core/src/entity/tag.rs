// r8s-core/src/entity/tag.rs
//! Define a entidade Tag, usada para categorização e organização.

use std::fmt;
use validator::Validate;

use crate::common::{EntityId, Identifiable, Result, Timestamp, Validatable};
use crate::error::Error;

/// Representa uma tag para categorização de objetos.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Validate)]
pub struct Tag {
    /// Identificador único da tag
    pub id: EntityId,

    /// Nome da tag
    #[validate(length(min = 1, max = 50))]
    pub name: String,

    /// Cor da tag para exibição visual
    pub color: TagColor,

    /// Descrição opcional da tag
    #[serde(default)]
    #[validate(length(max = 255))]
    pub description: Option<String>,

    /// ID do usuário que criou a tag
    pub created_by: Option<EntityId>,

    /// Se a tag é do sistema (não pode ser excluída)
    #[serde(default)]
    pub system_tag: bool,

    /// Timestamps de criação e atualização
    #[serde(default)]
    pub timestamps: Timestamp,
}

/// Cores disponíveis para tags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagColor {
    /// Vermelho
    Red,

    /// Verde
    Green,

    /// Azul
    Blue,

    /// Amarelo
    Yellow,

    /// Laranja
    Orange,

    /// Roxo
    Purple,

    /// Cinza
    Gray,

    /// Turquesa
    Teal,

    /// Rosa
    Pink,

    /// Marrom
    Brown,
}

impl Default for TagColor {
    fn default() -> Self {
        Self::Blue
    }
}

impl fmt::Display for TagColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let color = match self {
            TagColor::Red => "red",
            TagColor::Green => "green",
            TagColor::Blue => "blue",
            TagColor::Yellow => "yellow",
            TagColor::Orange => "orange",
            TagColor::Purple => "purple",
            TagColor::Gray => "gray",
            TagColor::Teal => "teal",
            TagColor::Pink => "pink",
            TagColor::Brown => "brown",
        };

        write!(f, "{}", color)
    }
}

impl TagColor {
    /// Retorna o código hexadecimal para a cor
    pub fn hex_code(&self) -> &'static str {
        match self {
            TagColor::Red => "#ff5252",
            TagColor::Green => "#4caf50",
            TagColor::Blue => "#2196f3",
            TagColor::Yellow => "#ffeb3b",
            TagColor::Orange => "#ff9800",
            TagColor::Purple => "#9c27b0",
            TagColor::Gray => "#9e9e9e",
            TagColor::Teal => "#009688",
            TagColor::Pink => "#e91e63",
            TagColor::Brown => "#795548",
        }
    }
}

impl Tag {
    /// Cria uma nova tag
    pub fn new(
        name: String,
        color: TagColor,
        description: Option<String>,
        created_by: Option<EntityId>,
    ) -> Self {
        Self {
            id: EntityId::new_v4(),
            name,
            color,
            description,
            created_by,
            system_tag: false,
            timestamps: Timestamp::new(),
        }
    }

    /// Cria uma nova tag do sistema (protegida)
    pub fn new_system_tag(name: String, color: TagColor, description: Option<String>) -> Self {
        let mut tag = Self::new(name, color, description, None);
        tag.system_tag = true;
        tag
    }

    /// Atualiza a descrição da tag
    pub fn update_description(&mut self, description: Option<String>) -> Result<()> {
        if self.system_tag {
            return Err(Error::Unauthorized(
                "Não é possível modificar uma tag do sistema".to_string(),
            ));
        }

        self.description = description;
        self.timestamps.update();
        Ok(())
    }

    /// Atualiza a cor da tag
    pub fn update_color(&mut self, color: TagColor) -> Result<()> {
        if self.system_tag {
            return Err(Error::Unauthorized(
                "Não é possível modificar uma tag do sistema".to_string(),
            ));
        }

        self.color = color;
        self.timestamps.update();
        Ok(())
    }
}

impl Identifiable for Tag {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Tag {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Validações adicionais específicas
        if self.name.trim().is_empty() {
            return Err(Error::Validation(
                "Nome da tag não pode ser vazio".to_string(),
            ));
        }

        Ok(())
    }
}
