// r8s-execution/src/runtime/js_runtime.rs
//! JavaScript runtime implementation using V8.
//!
//! This module provides an implementation of the JavaScript runtime interface
//! using the V8 JavaScript engine.

use std::sync::Arc;
use std::sync::Mutex;
use async_trait::async_trait;
use rusty_v8 as v8;

use crate::error::{ExecutionError, Result};
use crate::runtime::JSRuntime;

// Initialize V8 once when the module is loaded
lazy_static::lazy_static! {
    static ref V8_PLATFORM: v8::Platform = {
        let platform = v8::new_default_platform().unwrap();
        v8::V8::initialize_platform(platform.clone());
        v8::V8::initialize();
        platform
    };
}

/// V8 JavaScript runtime implementation.
pub struct V8JavaScriptRuntime {
    /// V8 isolate for executing JavaScript
    isolate: Mutex<v8::OwnedIsolate>,
    
    /// Global context for the isolate
    context: v8::Global<v8::Context>,
    
    /// Stored global values
    globals: serde_json::Map<String, serde_json::Value>,
}

impl V8JavaScriptRuntime {
    /// Create a new V8 JavaScript runtime.
    pub fn new() -> Self {
        // Ensure platform is initialized (will happen only once)
        lazy_static::initialize(&V8_PLATFORM);
        
        // Create a new isolate
        let mut isolate = v8::Isolate::new(Default::default());
        
        // Create a new context
        let mut context = {
            let scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Context::new(scope);
            v8::Global::new(scope, context)
        };
        
        Self {
            isolate: Mutex::new(isolate),
            context,
            globals: serde_json::Map::new(),
        }
    }
    
    /// Convert a Rust value to a V8 value.
    fn to_v8_value<'s>(
        &self,
        scope: &mut v8::HandleScope<'s>,
        value: &serde_json::Value,
    ) -> Result<v8::Local<'s, v8::Value>> {
        match value {
            serde_json::Value::Null => Ok(v8::null(scope).into()),
            serde_json::Value::Bool(b) => Ok(v8::Boolean::new(scope, *b).into()),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(v8::Integer::new(scope, i as i32).into())
                } else if let Some(f) = n.as_f64() {
                    Ok(v8::Number::new(scope, f).into())
                } else {
                    Err(ExecutionError::JavaScriptError("Invalid number value".to_string()))
                }
            },
            serde_json::Value::String(s) => {
                let v8_str = v8::String::new(scope, s).ok_or_else(|| {
                    ExecutionError::JavaScriptError("Failed to create V8 string".to_string())
                })?;
                Ok(v8_str.into())
            },
            serde_json::Value::Array(arr) => {
                let array = v8::Array::new(scope, arr.len() as i32);
                
                for (i, item) in arr.iter().enumerate() {
                    let v8_item = self.to_v8_value(scope, item)?;
                    array.set_index(scope, i as u32, v8_item);
                }
                
                Ok(array.into())
            },
            serde_json::Value::Object(obj) => {
                let object = v8::Object::new(scope);
                
                for (key, value) in obj {
                    let v8_key = v8::String::new(scope, key).ok_or_else(|| {
                        ExecutionError::JavaScriptError("Failed to create V8 string key".to_string())
                    })?;
                    
                    let v8_value = self.to_v8_value(scope, value)?;
                    object.set(scope, v8_key.into(), v8_value);
                }
                
                Ok(object.into())
            },
        }
    }
    
    /// Convert a V8 value to a Rust value.
    fn from_v8_value<'s>(
        &self,
        scope: &mut v8::HandleScope<'s>,
        value: v8::Local<'s, v8::Value>,
    ) -> Result<serde_json::Value> {
        if value.is_null() {
            return Ok(serde_json::Value::Null);
        } else if value.is_boolean() {
            let boolean = value.to_boolean(scope);
            return Ok(serde_json::Value::Bool(boolean.boolean_value(scope)));
        } else if value.is_int32() {
            let number = value.to_int32(scope).unwrap();
            return Ok(serde_json::Value::Number(number.value().into()));
        } else if value.is_number() {
            let number = value.to_number(scope).unwrap();
            let float = number.number_value(scope).unwrap();
            return Ok(serde_json::Value::Number(serde_json::Number::from_f64(float).unwrap_or(0.into())));
        } else if value.is_string() {
            let string = value.to_string(scope).unwrap();
            let rust_str = string.to_rust_string_lossy(scope);
            return Ok(serde_json::Value::String(rust_str));
        } else if value.is_array() {
            let array = value.to_object(scope).unwrap();
            let length = array.length();
            let mut result = Vec::with_capacity(length as usize);
            
            for i in 0..length {
                let item = array.get_index(scope, i).unwrap();
                result.push(self.from_v8_value(scope, item)?);
            }
            
            return Ok(serde_json::Value::Array(result));
        } else if value.is_object() {
            let object = value.to_object(scope).unwrap();
            let property_names = object.get_property_names(scope, Default::default()).unwrap();
            let mut result = serde_json::Map::new();
            
            for i in 0..property_names.length() {
                let key = property_names.get_index(scope, i).unwrap();
                let property_value = object.get(scope, key).unwrap();
                
                let key_str = key.to_string(scope).unwrap().to_rust_string_lossy(scope);
                result.insert(key_str, self.from_v8_value(scope, property_value)?);
            }
            
            return Ok(serde_json::Value::Object(result));
        } else if value.is_undefined() {
            return Ok(serde_json::Value::Null);
        }
        
        Err(ExecutionError::JavaScriptError("Unsupported V8 value type".to_string()))
    }
}

#[async_trait]
impl JSRuntime for V8JavaScriptRuntime {
    async fn execute_js(&mut self, code: &str) -> Result<serde_json::Value> {
        let mut isolate = self.isolate.lock().unwrap();
        
        // Create scope with context
        let mut scope = v8::HandleScope::with_context(&mut *isolate, self.context.clone());
        
        // Compile and run the code
        let source = v8::String::new(&mut scope, code).ok_or_else(|| {
            ExecutionError::JavaScriptError("Failed to create V8 source string".to_string())
        })?;
        
        let script = v8::Script::compile(&mut scope, source, None).ok_or_else(|| {
            ExecutionError::JavaScriptError("Failed to compile JavaScript code".to_string())
        })?;
        
        let result = script.run(&mut scope).ok_or_else(|| {
            ExecutionError::JavaScriptError("Failed to execute JavaScript code".to_string())
        })?;
        
        // Convert result to Rust value
        self.from_v8_value(&mut scope, result)
    }
    
    fn set_global(&mut self, name: &str, value: &serde_json::Value) -> Result<()> {
        // Store in our internal map for later retrieval
        self.globals.insert(name.to_string(), value.clone());
        
        let mut isolate = self.isolate.lock().unwrap();
        
        // Create scope with context
        let mut scope = v8::HandleScope::with_context(&mut *isolate, self.context.clone());
        
        // Get the global object
        let global = scope.get_current_context().global(&mut scope);
        
        // Convert name to V8 string
        let v8_name = v8::String::new(&mut scope, name).ok_or_else(|| {
            ExecutionError::JavaScriptError("Failed to create V8 name string".to_string())
        })?;
        
        // Convert value to V8 value
        let v8_value = self.to_v8_value(&mut scope, value)?;
        
        // Set global property
        global.set(&mut scope, v8_name.into(), v8_value);
        
        Ok(())
    }
    
    fn get_global<T: serde::de::DeserializeOwned>(&mut self, name: &str) -> Result<T> {
        // Try to get from our internal map first (for complex objects)
        if let Some(value) = self.globals.get(name) {
            return serde_json::from_value(value.clone())
                .map_err(|e| ExecutionError::JavaScriptError(format!("Failed to deserialize value: {}", e)));
        }
        
        let mut isolate = self.isolate.lock().unwrap();
        
        // Create scope with context
        let mut scope = v8::HandleScope::with_context(&mut *isolate, self.context.clone());
        
        // Get the global object
        let global = scope.get_current_context().global(&mut scope);
        
        // Convert name to V8 string
        let v8_name = v8::String::new(&mut scope, name).ok_or_else(|| {
            ExecutionError::JavaScriptError("Failed to create V8 name string".to_string())
        })?;
        
        // Get global property
        let value = global.get(&mut scope, v8_name.into()).ok_or_else(|| {
            ExecutionError::JavaScriptError(format!("Global '{}' not found", name))
        })?;
        
        // Convert V8 value to Rust value
        let rust_value = self.from_v8_value(&mut scope, value)?;
        
        // Deserialize to requested type
        serde_json::from_value(rust_value)
            .map_err(|e| ExecutionError::JavaScriptError(format!("Failed to deserialize value: {}", e)))
    }
}