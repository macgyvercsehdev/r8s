// r8s-execution/src/runtime/mod.rs
//! Runtime environments for executing code.
//!
//! This module provides interfaces and implementations for different runtime
//! environments used to execute code in workflow nodes.

mod js_runtime;
mod lua_runtime;
mod wasm_runtime;

use std::sync::Arc;
use async_trait::async_trait;

use crate::error::Result;

pub use js_runtime::JavaScriptRuntime;
pub use lua_runtime::LuaRuntime;
pub use wasm_runtime::WasmRuntime;

/// Interface for a JavaScript runtime.
#[async_trait]
pub trait JSRuntime: Send + Sync {
    /// Execute JavaScript code and return the result.
    async fn execute_js(&mut self, code: &str) -> Result<serde_json::Value>;
    
    /// Set a global variable in the JavaScript environment.
    fn set_global(&mut self, name: &str, value: &serde_json::Value) -> Result<()>;
    
    /// Get a global variable from the JavaScript environment.
    fn get_global<T: serde::de::DeserializeOwned>(&mut self, name: &str) -> Result<T>;
}

/// Interface for a Lua runtime.
#[async_trait]
pub trait LuaRuntime: Send + Sync {
    /// Execute Lua code and return the result.
    async fn execute_lua(&mut self, code: &str) -> Result<serde_json::Value>;
    
    /// Set a global variable in the Lua environment.
    fn set_global(&mut self, name: &str, value: &serde_json::Value) -> Result<()>;
    
    /// Get a global variable from the Lua environment.
    fn get_global<T: serde::de::DeserializeOwned>(&mut self, name: &str) -> Result<T>;
}

/// Interface for a WebAssembly runtime.
#[async_trait]
pub trait WasmRuntime: Send + Sync {
    /// Execute a WebAssembly module with the given function and arguments.
    async fn execute_wasm(&mut self, wasm_bytes: &[u8], function: &str, args: &[String]) -> Result<String>;
}

/// Factory for creating runtime environments.
#[async_trait]
pub trait RuntimeFactory: Send + Sync {
    /// Create a new JavaScript runtime.
    async fn create_js_runtime(&self) -> Result<Box<dyn JSRuntime>>;
    
    /// Create a new Lua runtime.
    async fn create_lua_runtime(&self) -> Result<Box<dyn LuaRuntime>>;
    
    /// Create a new WebAssembly runtime.
    async fn create_wasm_runtime(&self) -> Result<Box<dyn WasmRuntime>>;
}

/// Default runtime factory implementation.
pub struct DefaultRuntimeFactory {}

impl DefaultRuntimeFactory {
    /// Create a new default runtime factory.
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl RuntimeFactory for DefaultRuntimeFactory {
    async fn create_js_runtime(&self) -> Result<Box<dyn JSRuntime>> {
        Ok(Box::new(js_runtime::V8JavaScriptRuntime::new()))
    }
    
    async fn create_lua_runtime(&self) -> Result<Box<dyn LuaRuntime>> {
        Ok(Box::new(lua_runtime::MLuaRuntime::new()))
    }
    
    async fn create_wasm_runtime(&self) -> Result<Box<dyn WasmRuntime>> {
        Ok(Box::new(wasm_runtime::WasmerRuntime::new()))
    }
}