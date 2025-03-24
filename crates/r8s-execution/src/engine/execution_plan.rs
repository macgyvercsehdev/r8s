// r8s-execution/src/engine/execution_plan.rs
//! Execution plan for workflows.
//!
//! This module provides functionality to analyze a workflow and create an execution
//! plan that determines the order of node execution and dependencies.

use crate::error::{ExecutionError, Result};
use r8s_core::common::EntityId;
use r8s_core::entity::Workflow;
use std::collections::{HashMap, HashSet};

/// Represents an execution plan for a workflow.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Node dependencies (nodes that must execute before this node)
    dependencies: HashMap<EntityId, HashSet<EntityId>>,

    /// Node dependents (nodes that depend on this node)
    dependents: HashMap<EntityId, HashSet<EntityId>>,

    /// Nodes without dependencies (start nodes)
    initial_nodes: HashSet<EntityId>,

    /// Nodes without dependents (end nodes)
    final_nodes: HashSet<EntityId>,
}

impl ExecutionPlan {
    /// Create a new execution plan from a workflow.
    pub fn new(workflow: &Workflow) -> Result<Self> {
        let mut dependencies: HashMap<EntityId, HashSet<EntityId>> = HashMap::new();
        let mut dependents: HashMap<EntityId, HashSet<EntityId>> = HashMap::new();

        // Initialize empty sets for all nodes
        for node in &workflow.nodes {
            dependencies.insert(node.id, HashSet::new());
            dependents.insert(node.id, HashSet::new());
        }

        // Process all connections to build the dependency graph
        for connection in &workflow.connections {
            // Skip connections from disabled nodes
            if let Some(source_node) = workflow
                .nodes
                .iter()
                .find(|n| n.id == connection.source_node)
            {
                if source_node.disabled {
                    continue;
                }
            }

            // Skip connections to disabled nodes
            if let Some(target_node) = workflow
                .nodes
                .iter()
                .find(|n| n.id == connection.target_node)
            {
                if target_node.disabled {
                    continue;
                }
            }

            // Add dependency: target depends on source
            if let Some(deps) = dependencies.get_mut(&connection.target_node) {
                deps.insert(connection.source_node);
            }

            // Add dependent: source has target as dependent
            if let Some(deps) = dependents.get_mut(&connection.source_node) {
                deps.insert(connection.target_node);
            }
        }

        // Find initial nodes (no dependencies)
        let initial_nodes = dependencies
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(node_id, _)| *node_id)
            .collect::<HashSet<_>>();

        // Find final nodes (no dependents)
        let final_nodes = dependents
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(node_id, _)| *node_id)
            .collect::<HashSet<_>>();

        // Ensure we have at least one initial node
        if initial_nodes.is_empty() {
            return Err(ExecutionError::ExecutionError(
                "Workflow has no starting nodes".to_string(),
            ));
        }

        // Check for cycles in the dependency graph
        if Self::has_cycles(&dependencies) {
            return Err(ExecutionError::ExecutionError(
                "Workflow contains cycles, which are not supported".to_string(),
            ));
        }

        Ok(Self {
            dependencies,
            dependents,
            initial_nodes,
            final_nodes,
        })
    }

    /// Check if the dependency graph has cycles using depth-first search.
    fn has_cycles(dependencies: &HashMap<EntityId, HashSet<EntityId>>) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node_id in dependencies.keys() {
            if !visited.contains(node_id) {
                if Self::is_cyclic(dependencies, *node_id, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }

        false
    }

    /// Helper function for cycle detection using DFS.
    fn is_cyclic(
        dependencies: &HashMap<EntityId, HashSet<EntityId>>,
        node_id: EntityId,
        visited: &mut HashSet<EntityId>,
        rec_stack: &mut HashSet<EntityId>,
    ) -> bool {
        // Mark the current node as visited and add to recursion stack
        visited.insert(node_id);
        rec_stack.insert(node_id);

        // Recur for all nodes dependent on this node
        if let Some(dependents) = dependencies.get(&node_id) {
            for dependent in dependents {
                // If the dependent is not visited, recursively check it
                if !visited.contains(dependent) {
                    if Self::is_cyclic(dependencies, *dependent, visited, rec_stack) {
                        return true;
                    }
                }
                // If the dependent is in the recursion stack, we found a cycle
                else if rec_stack.contains(dependent) {
                    return true;
                }
            }
        }

        // Remove node from recursion stack
        rec_stack.remove(&node_id);

        false
    }

    /// Get nodes that can be executed at the start (no dependencies).
    pub fn get_initial_nodes(&self) -> HashSet<EntityId> {
        self.initial_nodes.clone()
    }

    /// Get nodes that are execution endpoints (no dependents).
    pub fn get_final_nodes(&self) -> HashSet<EntityId> {
        self.final_nodes.clone()
    }

    /// Check if a node is an initial node (no dependencies).
    pub fn is_initial_node(&self, node_id: EntityId) -> bool {
        self.initial_nodes.contains(&node_id)
    }

    /// Check if a node is a final node (no dependents).
    pub fn is_final_node(&self, node_id: EntityId) -> bool {
        self.final_nodes.contains(&node_id)
    }

    /// Get dependencies for a node (nodes that must execute before this node).
    pub fn get_node_dependencies(&self, node_id: EntityId) -> HashSet<EntityId> {
        self.dependencies.get(&node_id).cloned().unwrap_or_default()
    }

    /// Get dependent nodes (nodes that depend on this node).
    pub fn get_dependent_nodes(&self, node_id: EntityId) -> HashSet<EntityId> {
        self.dependents.get(&node_id).cloned().unwrap_or_default()
    }

    /// Get all nodes in the plan.
    pub fn get_all_nodes(&self) -> HashSet<EntityId> {
        self.dependencies.keys().copied().collect()
    }
}
