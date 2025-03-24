// r8s-execution/src/runtime/wasm_runtime.rs
//! WebAssembly runtime implementation.
//!
//! This module provides an implementation of the WebAssembly runtime interface
//! using the Wasmer library.

use std::sync::Arc;
use async_trait::async_trait;
use wasmer::{Instance, Module, Store, Value, Engine, Universal, Function, FunctionType};
use wasmer::{ImportObject, Exports, WasmPtr, Memory, MemoryView};
use wasmer_wasi::{WasiState, WasiEnv};

use crate::error::{ExecutionError, Result};
use crate::runtime::WasmRuntime;

/// Wasmer WebAssembly runtime implementation.
pub struct WasmerRuntime {
    /// Wasmer store
    store: Store,
    
    /// Current module instance (if any)
    instance: Option<Instance>,
    
    /// WASI environment
    wasi_env: Option<WasiEnv>,
}

impl WasmerRuntime {
    /// Create a new WebAssembly runtime.
    pub fn new() -> Self {
        // Create a new engine
        let engine = Universal::new(wasmer::Cranelift::default()).engine();
        
        // Create a store
        let store = Store::new(&engine);
        
        Self {
            store,
            instance: None,
            wasi_env: None,
        }
    }
    
    /// Initialize a WASI environment.
    fn init_wasi(&mut self) -> Result<()> {
        // Create a WASI environment
        let wasi_env = WasiState::new("r8s")
            .map_dir("/", ".")? // Map the root directory
            .finalize()?;
        
        self.wasi_env = Some(wasi_env);
        
        Ok(())
    }
    
    /// Load and instantiate a WebAssembly module.
    fn load_module(&mut self, wasm_bytes: &[u8]) -> Result<()> {
        // Compile the module
        let module = Module::new(&self.store, wasm_bytes)
            .map_err(|e| ExecutionError::WasmError(format!("Failed to compile WebAssembly module: {}", e)))?;
        
        // Initialize WASI if needed
        if self.wasi_env.is_none() {
            self.init_wasi()?;
        }
        
        // Get WASI import object
        let import_object = if let Some(wasi_env) = &self.wasi_env {
            wasi_env.import_object(&self.store, &module)
                .map_err(|e| ExecutionError::WasmError(format!("Failed to create WASI imports: {}", e)))?
        } else {
            ImportObject::new()
        };
        
        // Instantiate the module
        let instance = Instance::new(&module, &import_object)
            .map_err(|e| ExecutionError::WasmError(format!("Failed to instantiate WebAssembly module: {}", e)))?;
        
        self.instance = Some(instance);
        
        Ok(())
    }
    
    /// Call a function with string arguments and return a string result.
    fn call_string_function(&mut self, function_name: &str, args: &[String]) -> Result<String> {
        let instance = self.instance.as_ref()
            .ok_or_else(|| ExecutionError::WasmError("No WebAssembly module loaded".to_string()))?;
        
        // Check if the function exists
        if !instance.exports.contains(function_name) {
            return Err(ExecutionError::WasmError(
                format!("Function '{}' not found in WebAssembly module", function_name)
            ));
        }
        
        // Get memory
        let memory = instance.exports.get_memory("memory")
            .map_err(|e| ExecutionError::WasmError(format!("Failed to get WebAssembly memory: {}", e)))?;
        
        // Get the function
        let function = instance.exports.get_function(function_name)
            .map_err(|e| ExecutionError::WasmError(format!("Failed to get function '{}': {}", function_name, e)))?;
        
        // We need to allocate memory for the input strings
        let alloc_function = instance.exports.get_function("alloc")
            .map_err(|_| ExecutionError::WasmError("WebAssembly module must export 'alloc' function".to_string()))?;
        
        // Prepare arguments
        let mut wasm_args = Vec::new();
        let mut allocated_ptrs = Vec::new();
        
        // For each string argument, allocate memory and write the string
        for arg in args {
            // Allocate memory for the string
            let alloc_result = alloc_function.call(&[Value::I32(arg.len() as i32 + 1)])
                .map_err(|e| ExecutionError::WasmError(format!("Failed to allocate memory: {}", e)))?;
            
            let ptr = alloc_result[0].unwrap_i32() as u32;
            allocated_ptrs.push(ptr);
            
            // Write the string to memory
            let memory_view: MemoryView<u8> = memory.view();
            for (i, byte) in arg.bytes().enumerate() {
                memory_view.write(ptr + i as u32, byte)
                    .map_err(|e| ExecutionError::WasmError(format!("Failed to write to memory: {}", e)))?;
            }
            
            // Null terminate
            memory_view.write(ptr + arg.len() as u32, 0)
                .map_err(|e| ExecutionError::WasmError(format!("Failed to write to memory: {}", e)))?;
            
            // Add pointer to arguments
            wasm_args.push(Value::I32(ptr as i32));
        }
        
        // Call the function
        let result = function.call(&wasm_args)
            .map_err(|e| ExecutionError::WasmError(format!("Failed to call function: {}", e)))?;
        
        // Get the result pointer
        let result_ptr = result[0].unwrap_i32() as u32;
        
        // Read the result string from memory
        let memory_view: MemoryView<u8> = memory.view();
        let mut result_bytes = Vec::new();
        let mut i = 0;
        
        // Read until null terminator
        loop {
            let byte = memory_view.read(result_ptr + i)
                .map_err(|e| ExecutionError::WasmError(format!("Failed to read from memory: {}", e)))?;
            
            if byte == 0 {
                break;
            }
            
            result_bytes.push(byte);
            i += 1;
        }
        
        // Convert to string
        let result_string = String::from_utf8(result_bytes)
            .map_err(|e| ExecutionError::WasmError(format!("Invalid UTF-8 in result: {}", e)))?;
        
        // Free the result memory (if the module exports a free function)
        if let Ok(free_function) = instance.exports.get_function("free") {
            free_function.call(&[Value::I32(result_ptr as i32)])
                .map_err(|e| ExecutionError::WasmError(format!("Failed to free result memory: {}", e)))?;
            
            // Free the argument memory
            for ptr in allocated_ptrs {
                free_function.call(&[Value::I32(ptr as i32)])
                    .map_err(|e| ExecutionError::WasmError(format!("Failed to free argument memory: {}", e)))?;
            }
        }
        
        Ok(result_string)
    }
}

#[async_trait]
impl WasmRuntime for WasmerRuntime {
    async fn execute_wasm(&mut self, wasm_bytes: &[u8], function: &str, args: &[String]) -> Result<String> {
        // Load the module
        self.load_module(wasm_bytes)?;
        
        // Call the function
        self.call_string_function(function, args)
    }
}