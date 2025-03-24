// r8s-execution/src/runtime/lua_runtime.rs
//! Lua runtime implementation.
//!
//! This module provides an implementation of the Lua runtime interface
//! using the MLua library.

use std::sync::Arc;
use async_trait::async_trait;
use mlua::{Lua, LuaOptions, Value as LuaValue, Function};

use crate::error::{ExecutionError, Result};
use crate::runtime::LuaRuntime;

/// MLua Lua runtime implementation.
pub struct MLuaRuntime {
    /// Lua state
    lua: Lua,
}

impl MLuaRuntime {
    /// Create a new Lua runtime.
    pub fn new() -> Self {
        // Create new Lua state with safe settings
        let lua = Lua::new_with(
            LuaOptions::new()
                .catch_rust_panics(true)
                .allow_unsafe_functions(false)
        ).expect("Failed to create Lua state");
        
        let runtime = Self { lua };
        
        // Set up JSON library
        runtime.setup_json_lib();
        
        runtime
    }
    
    /// Set up JSON library for Lua.
    fn setup_json_lib(&self) {
        // Add JSON encode/decode functions
        let globals = self.lua.globals();
        
        let json_table = self.lua.create_table().expect("Failed to create JSON table");
        
        // Encode function
        let encode = self.lua.create_function(|_, value: LuaValue| {
            let json_str = match serde_json::to_string(&lua_to_json_value(value)) {
                Ok(s) => s,
                Err(e) => return Err(mlua::Error::RuntimeError(format!("JSON encode error: {}", e))),
            };
            Ok(json_str)
        }).expect("Failed to create encode function");
        
        // Decode function
        let decode = self.lua.create_function(|lua, json_str: String| {
            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(value) => {
                    match json_to_lua_value(lua, &value) {
                        Ok(lua_value) => Ok(lua_value),
                        Err(e) => Err(mlua::Error::RuntimeError(format!("JSON decode error: {}", e))),
                    }
                },
                Err(e) => Err(mlua::Error::RuntimeError(format!("JSON parse error: {}", e))),
            }
        }).expect("Failed to create decode function");
        
        json_table.set("encode", encode).expect("Failed to set encode function");
        json_table.set("decode", decode).expect("Failed to set decode function");
        
        globals.set("json", json_table).expect("Failed to set JSON table");
    }
    
    /// Convert a Rust value to a Lua value.
    fn to_lua_value(&self, value: &serde_json::Value) -> Result<LuaValue> {
        json_to_lua_value(&self.lua, value)
            .map_err(|e| ExecutionError::LuaError(format!("Failed to convert to Lua value: {}", e)))
    }
    
    /// Convert a Lua value to a Rust value.
    fn from_lua_value(&self, value: LuaValue) -> Result<serde_json::Value> {
        Ok(lua_to_json_value(value))
    }
}

/// Convert a Lua value to a JSON value.
fn lua_to_json_value(value: LuaValue) -> serde_json::Value {
    match value {
        LuaValue::Nil => serde_json::Value::Null,
        LuaValue::Boolean(b) => serde_json::Value::Bool(b),
        LuaValue::Integer(i) => serde_json::Value::Number(i.into()),
        LuaValue::Number(n) => {
            match serde_json::Number::from_f64(n) {
                Some(num) => serde_json::Value::Number(num),
                None => serde_json::Value::Null,
            }
        },
        LuaValue::String(s) => {
            match s.to_str() {
                Ok(s) => serde_json::Value::String(s.to_string()),
                Err(_) => serde_json::Value::Null,
            }
        },
        LuaValue::Table(t) => {
            // Check if the table is an array
            let len = t.len().unwrap_or(0);
            
            if len > 0 {
                // It's an array
                let mut array = Vec::with_capacity(len as usize);
                
                for i in 1..=len {
                    let value = match t.get::<_, LuaValue>(i) {
                        Ok(v) => lua_to_json_value(v),
                        Err(_) => serde_json::Value::Null,
                    };
                    array.push(value);
                }
                
                serde_json::Value::Array(array)
            } else {
                // It's an object
                let mut map = serde_json::Map::new();
                
                // Iterate through the table keys
                if let Ok(pairs) = t.pairs::<LuaValue, LuaValue>() {
                    for pair in pairs {
                        if let Ok((key, value)) = pair {
                            // Only string keys are supported
                            let key_str = match key {
                                LuaValue::String(s) => {
                                    match s.to_str() {
                                        Ok(s) => s.to_string(),
                                        Err(_) => continue,
                                    }
                                },
                                _ => continue,
                            };
                            
                            map.insert(key_str, lua_to_json_value(value));
                        }
                    }
                }
                
                serde_json::Value::Object(map)
            }
        },
        _ => serde_json::Value::Null,
    }
}

/// Convert a JSON value to a Lua value.
fn json_to_lua_value(lua: &Lua, value: &serde_json::Value) -> mlua::Result<LuaValue> {
    match value {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Ok(LuaValue::Nil)
            }
        },
        serde_json::Value::String(s) => {
            let lua_string = lua.create_string(s)?;
            Ok(LuaValue::String(lua_string))
        },
        serde_json::Value::Array(a) => {
            let table = lua.create_table()?;
            
            for (i, value) in a.iter().enumerate() {
                table.set(i + 1, json_to_lua_value(lua, value)?)?;
            }
            
            Ok(LuaValue::Table(table))
        },
        serde_json::Value::Object(o) => {
            let table = lua.create_table()?;
            
            for (key, value) in o {
                table.set(key.clone(), json_to_lua_value(lua, value)?)?;
            }
            
            Ok(LuaValue::Table(table))
        },
    }
}

#[async_trait]
impl LuaRuntime for MLuaRuntime {
    async fn execute_lua(&mut self, code: &str) -> Result<serde_json::Value> {
        match self.lua.load(code).eval::<LuaValue>() {
            Ok(result) => self.from_lua_value(result),
            Err(err) => Err(ExecutionError::LuaError(format!("Lua execution error: {}", err))),
        }
    }
    
    fn set_global(&mut self, name: &str, value: &serde_json::Value) -> Result<()> {
        let lua_value = self.to_lua_value(value)?;
        
        match self.lua.globals().set(name, lua_value) {
            Ok(_) => Ok(()),
            Err(err) => Err(ExecutionError::LuaError(
                format!("Failed to set global '{}': {}", name, err)
            )),
        }
    }
    
    fn get_global<T: serde::de::DeserializeOwned>(&mut self, name: &str) -> Result<T> {
        let lua_value = match self.lua.globals().get::<_, LuaValue>(name) {
            Ok(value) => value,
            Err(err) => return Err(ExecutionError::LuaError(
                format!("Failed to get global '{}': {}", name, err)
            )),
        };
        
        let json_value = self.from_lua_value(lua_value)?;
        
        match serde_json::from_value(json_value) {
            Ok(value) => Ok(value),
            Err(err) => Err(ExecutionError::LuaError(
                format!("Failed to deserialize global '{}': {}", name, err)
            )),
        }
    }
}