// r8s-core/src/use_case/variable.rs
//! Casos de uso relacionados a variáveis.

use std::sync::Arc;

use crate::common::{EntityId, Result};
use crate::entity::{Variable, VariableType, VariableScope, User};
use crate::error::Error;
use crate::repository::{VariableRepository, UserRepository};

/// Caso de uso para criar uma nova variável
pub struct CreateVariableUseCase<R: VariableRepository, U: UserRepository> {
    variable_repository: Arc<R>,
    user_repository: Arc<U>,
}

/// Entrada para o caso de uso de criação de variável
pub struct CreateVariableInput {
    /// Chave da variável
    pub key: String,
    
    /// Valor da variável
    pub value: String,
    
    /// Tipo da variável
    pub var_type: VariableType,
    
    /// Escopo da variável
    pub scope: VariableScope,
    
    /// Se a variável é protegida (sensível)
    pub protected: bool,
    
    /// Descrição opcional
    pub description: Option<String>,
    
    /// ID do usuário que está criando a variável
    pub user_id: EntityId,
}

impl<R: VariableRepository, U: UserRepository> CreateVariableUseCase<R, U> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>, user_repository: Arc<U>) -> Self {
        Self {
            variable_repository,
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: CreateVariableInput) -> Result<Variable> {
        // Verifica se o usuário existe
        let user = self.user_repository.find_by_id(input.user_id).await?;
        
        // Verifica permissões com base no escopo
        match &input.scope {
            // Variáveis globais só podem ser criadas por administradores
            VariableScope::Global => {
                if !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Apenas administradores podem criar variáveis globais".to_string()
                    ));
                }
            },
            
            // Variáveis de workflow só podem ser criadas por administradores ou donos do workflow
            VariableScope::Workflow(_) => {
                // Aqui seria necessário verificar se o usuário tem acesso ao workflow
                // Mas isso requer acessar o repositório de workflows, que não temos aqui
                // Para simplificar, vamos assumir que usuários com papel PowerUser ou superior podem criar
                if !user.is_team_admin() {
                    return Err(Error::Unauthorized(
                        "Permissão insuficiente para criar variáveis de workflow".to_string()
                    ));
                }
            },
            
            // Variáveis de usuário só podem ser criadas pelo próprio usuário ou administradores
            VariableScope::User(scope_user_id) => {
                if *scope_user_id != user.id && !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Você não pode criar variáveis para outro usuário".to_string()
                    ));
                }
            },
            
            // Variáveis de ambiente só podem ser criadas por administradores
            VariableScope::Environment(_) => {
                if !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Apenas administradores podem criar variáveis de ambiente".to_string()
                    ));
                }
            },
        }
        
        // Verifica se já existe uma variável com a mesma chave no mesmo escopo
        let existing = self.variable_repository
            .find_by_scope_and_key(&input.scope, &input.key)
            .await?;
            
        if existing.is_some() {
            return Err(Error::Conflict(
                format!("Já existe uma variável com a chave '{}' neste escopo", input.key)
            ));
        }
        
        // Cria a variável
        let mut variable = Variable::new(
            input.key,
            input.value,
            input.var_type,
            input.scope,
            input.protected,
        );
        
        // Define a descrição, se fornecida
        variable.description = input.description;
        
        // Valida a variável
        variable.validate()?;
        
        // Salva a variável
        let variable = self.variable_repository.save(variable).await?;
        
        Ok(variable)
    }
}

/// Caso de uso para obter uma variável
pub struct GetVariableUseCase<R: VariableRepository> {
    variable_repository: Arc<R>,
}

impl<R: VariableRepository> GetVariableUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>) -> Self {
        Self {
            variable_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, variable_id: EntityId, user_id: EntityId) -> Result<Variable> {
        // Verifica se o usuário tem acesso à variável
        let has_access = self.variable_repository.has_access(variable_id, user_id).await?;
        
        if !has_access {
            return Err(Error::Unauthorized(
                format!("Usuário não tem acesso à variável {}", variable_id)
            ));
        }
        
        // Obtém a variável
        let variable = self.variable_repository.find_by_id(variable_id).await?;
        
        Ok(variable)
    }
}

/// Caso de uso para atualizar o valor de uma variável
pub struct UpdateVariableValueUseCase<R: VariableRepository, U: UserRepository> {
    variable_repository: Arc<R>,
    user_repository: Arc<U>,
}

/// Entrada para o caso de uso de atualização de valor
pub struct UpdateVariableValueInput {
    /// ID da variável
    pub variable_id: EntityId,
    
    /// Novo valor
    pub value: String,
    
    /// ID do usuário que está atualizando
    pub user_id: EntityId,
}

impl<R: VariableRepository, U: UserRepository> UpdateVariableValueUseCase<R, U> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>, user_repository: Arc<U>) -> Self {
        Self {
            variable_repository,
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: UpdateVariableValueInput) -> Result<Variable> {
        // Busca a variável
        let mut variable = self.variable_repository.find_by_id(input.variable_id).await?;
        
        // Busca o usuário
        let user = self.user_repository.find_by_id(input.user_id).await?;
        
        // Verifica permissões com base no escopo
        match &variable.scope {
            // Variáveis globais só podem ser atualizadas por administradores
            VariableScope::Global => {
                if !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Apenas administradores podem atualizar variáveis globais".to_string()
                    ));
                }
            },
            
            // Variáveis de workflow só podem ser atualizadas por administradores ou donos do workflow
            VariableScope::Workflow(_) => {
                if !user.is_team_admin() {
                    return Err(Error::Unauthorized(
                        "Permissão insuficiente para atualizar variáveis de workflow".to_string()
                    ));
                }
            },
            
            // Variáveis de usuário só podem ser atualizadas pelo próprio usuário ou administradores
            VariableScope::User(scope_user_id) => {
                if *scope_user_id != user.id && !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Você não pode atualizar variáveis de outro usuário".to_string()
                    ));
                }
            },
            
            // Variáveis de ambiente só podem ser atualizadas por administradores
            VariableScope::Environment(_) => {
                if !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Apenas administradores podem atualizar variáveis de ambiente".to_string()
                    ));
                }
            },
        }
        
        // Atualiza o valor
        variable.update_value(input.value);
        
        // Valida a variável
        variable.validate()?;
        
        // Salva as alterações
        let variable = self.variable_repository.save(variable).await?;
        
        Ok(variable)
    }
}

/// Caso de uso para listar variáveis globais
pub struct ListGlobalVariablesUseCase<R: VariableRepository> {
    variable_repository: Arc<R>,
}

impl<R: VariableRepository> ListGlobalVariablesUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>) -> Self {
        Self {
            variable_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self) -> Result<Vec<Variable>> {
        let variables = self.variable_repository.find_global_variables().await?;
        Ok(variables)
    }
}

/// Caso de uso para listar variáveis de um workflow
pub struct ListWorkflowVariablesUseCase<R: VariableRepository> {
    variable_repository: Arc<R>,
}

impl<R: VariableRepository> ListWorkflowVariablesUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>) -> Self {
        Self {
            variable_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, workflow_id: EntityId) -> Result<Vec<Variable>> {
        let variables = self.variable_repository.find_by_workflow(workflow_id).await?;
        Ok(variables)
    }
}

/// Caso de uso para listar variáveis de um usuário
pub struct ListUserVariablesUseCase<R: VariableRepository> {
    variable_repository: Arc<R>,
}

impl<R: VariableRepository> ListUserVariablesUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>) -> Self {
        Self {
            variable_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, user_id: EntityId) -> Result<Vec<Variable>> {
        let variables = self.variable_repository.find_by_user(user_id).await?;
        Ok(variables)
    }
}

/// Caso de uso para excluir uma variável
pub struct DeleteVariableUseCase<R: VariableRepository, U: UserRepository> {
    variable_repository: Arc<R>,
    user_repository: Arc<U>,
}

impl<R: VariableRepository, U: UserRepository> DeleteVariableUseCase<R, U> {
    /// Cria uma nova instância do caso de uso
    pub fn new(variable_repository: Arc<R>, user_repository: Arc<U>) -> Self {
        Self {
            variable_repository,
            user_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, variable_id: EntityId, user_id: EntityId) -> Result<()> {
        // Busca a variável
        let variable = self.variable_repository.find_by_id(variable_id).await?;
        
        // Busca o usuário
        let user = self.user_repository.find_by_id(user_id).await?;
        
        // Verifica permissões com base no escopo
        match &variable.scope {
            // Variáveis globais só podem ser excluídas por administradores
            VariableScope::Global => {
                if !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Apenas administradores podem excluir variáveis globais".to_string()
                    ));
                }
            },
            
            // Variáveis de workflow só podem ser excluídas por administradores ou donos do workflow
            VariableScope::Workflow(_) => {
                if !user.is_team_admin() {
                    return Err(Error::Unauthorized(
                        "Permissão insuficiente para excluir variáveis de workflow".to_string()
                    ));
                }
            },
            
            // Variáveis de usuário só podem ser excluídas pelo próprio usuário ou administradores
            VariableScope::User(scope_user_id) => {
                if *scope_user_id != user.id && !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Você não pode excluir variáveis de outro usuário".to_string()
                    ));
                }
            },
            
            // Variáveis de ambiente só podem ser excluídas por administradores
            VariableScope::Environment(_) => {
                if !user.is_admin() {
                    return Err(Error::Unauthorized(
                        "Apenas administradores podem excluir variáveis de ambiente".to_string()
                    ));
                }
            },
        }
        
        // Exclui a variável
        self.variable_repository.delete(variable_id).await?;
        
        Ok(())
    }
}