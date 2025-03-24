// r8s-core/src/use_case/workflow.rs
//! Casos de uso relacionados a workflows.

use std::sync::Arc;

use crate::common::{EntityId, Result, AsyncResult};
use crate::entity::{Workflow, Tag, Node, Connection};
use crate::error::Error;
use crate::repository::WorkflowRepository;

/// Caso de uso para criar um novo workflow
pub struct CreateWorkflowUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

/// Entrada para o caso de uso de criação de workflow
pub struct CreateWorkflowInput {
    /// Nome do workflow
    pub name: String,
    
    /// Descrição do workflow
    pub description: Option<String>,
    
    /// ID do usuário criador
    pub created_by: EntityId,
    
    /// Tags para associar ao workflow
    pub tags: Vec<Tag>,
}

impl<R: WorkflowRepository> CreateWorkflowUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: CreateWorkflowInput) -> Result<Workflow> {
        // Cria um novo workflow
        let mut workflow = Workflow::new(input.name, input.description);
        
        // Salva o workflow
        let workflow = self.workflow_repository.save(workflow).await?;
        
        // Adiciona as tags
        for tag in input.tags {
            self.workflow_repository.add_tag(workflow.id, &tag).await?;
        }
        
        // Retorna o workflow criado
        Ok(workflow)
    }
}

/// Caso de uso para atualizar um workflow
pub struct UpdateWorkflowUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

/// Entrada para o caso de uso de atualização de workflow
pub struct UpdateWorkflowInput {
    /// ID do workflow a ser atualizado
    pub id: EntityId,
    
    /// Novo nome (opcional)
    pub name: Option<String>,
    
    /// Nova descrição (opcional)
    pub description: Option<String>,
    
    /// Novo estado ativo (opcional)
    pub active: Option<bool>,
    
    /// Novas configurações (opcional)
    pub settings: Option<crate::entity::WorkflowSettings>,
    
    /// ID do usuário que está realizando a atualização
    pub updated_by: EntityId,
}

impl<R: WorkflowRepository> UpdateWorkflowUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: UpdateWorkflowInput) -> Result<Workflow> {
        // Busca o workflow existente
        let mut workflow = self.workflow_repository.find_by_id(input.id).await?;
        
        // Atualiza os campos
        if let Some(name) = input.name {
            workflow.name = name;
        }
        
        if let Some(description) = input.description {
            workflow.description = Some(description);
        }
        
        if let Some(active) = input.active {
            workflow.active = active;
        }
        
        if let Some(settings) = input.settings {
            workflow.settings = settings;
        }
        
        // Incrementa versão
        workflow.increment_version();
        
        // Salva as alterações
        let workflow = self.workflow_repository.save(workflow).await?;
        
        Ok(workflow)
    }
}

/// Caso de uso para adicionar um nó a um workflow
pub struct AddNodeUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

impl<R: WorkflowRepository> AddNodeUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, workflow_id: EntityId, node: Node) -> Result<Workflow> {
        // Busca o workflow existente
        let mut workflow = self.workflow_repository.find_by_id(workflow_id).await?;
        
        // Adiciona o nó
        workflow.add_node(node)?;
        
        // Incrementa versão
        workflow.increment_version();
        
        // Salva as alterações
        let workflow = self.workflow_repository.save(workflow).await?;
        
        Ok(workflow)
    }
}

/// Caso de uso para adicionar uma conexão a um workflow
pub struct AddConnectionUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

impl<R: WorkflowRepository> AddConnectionUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, workflow_id: EntityId, connection: Connection) -> Result<Workflow> {
        // Busca o workflow existente
        let mut workflow = self.workflow_repository.find_by_id(workflow_id).await?;
        
        // Adiciona a conexão
        workflow.add_connection(connection)?;
        
        // Incrementa versão
        workflow.increment_version();
        
        // Salva as alterações
        let workflow = self.workflow_repository.save(workflow).await?;
        
        Ok(workflow)
    }
}

/// Caso de uso para remover um nó de um workflow
pub struct RemoveNodeUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

impl<R: WorkflowRepository> RemoveNodeUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, workflow_id: EntityId, node_id: EntityId) -> Result<Workflow> {
        // Busca o workflow existente
        let mut workflow = self.workflow_repository.find_by_id(workflow_id).await?;
        
        // Remove o nó
        workflow.remove_node(node_id)?;
        
        // Incrementa versão
        workflow.increment_version();
        
        // Salva as alterações
        let workflow = self.workflow_repository.save(workflow).await?;
        
        Ok(workflow)
    }
}

/// Caso de uso para ativar um workflow
pub struct ActivateWorkflowUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

impl<R: WorkflowRepository> ActivateWorkflowUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, workflow_id: EntityId) -> Result<Workflow> {
        // Busca o workflow existente
        let mut workflow = self.workflow_repository.find_by_id(workflow_id).await?;
        
        // Realiza validações adicionais antes de ativar
        workflow.validate()?;
        
        // Um workflow precisa ter pelo menos um nó para ser ativo
        if workflow.nodes.is_empty() {
            return Err(Error::Validation("Workflow precisa ter ao menos um nó para ser ativado".to_string()));
        }
        
        // Ativa o workflow
        workflow.active = true;
        
        // Salva as alterações
        let workflow = self.workflow_repository.save(workflow).await?;
        
        Ok(workflow)
    }
}

/// Caso de uso para desativar um workflow
pub struct DeactivateWorkflowUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

impl<R: WorkflowRepository> DeactivateWorkflowUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, workflow_id: EntityId) -> Result<Workflow> {
        // Busca o workflow existente
        let mut workflow = self.workflow_repository.find_by_id(workflow_id).await?;
        
        // Desativa o workflow
        workflow.active = false;
        
        // Salva as alterações
        let workflow = self.workflow_repository.save(workflow).await?;
        
        Ok(workflow)
    }
}

/// Caso de uso para listar workflows com filtros
pub struct ListWorkflowsUseCase<R: WorkflowRepository> {
    workflow_repository: Arc<R>,
}

impl<R: WorkflowRepository> ListWorkflowsUseCase<R> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<R>) -> Self {
        Self {
            workflow_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, filter: crate::repository::workflow_repository::WorkflowFilter) -> Result<Vec<Workflow>> {
        let workflows = self.workflow_repository.find_by_filter(filter).await?;
        Ok(workflows)
    }
}