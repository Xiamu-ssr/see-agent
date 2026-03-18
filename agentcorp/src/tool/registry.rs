use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::{Result, AgentCorpError};
use crate::types::{FunctionSchema, ToolResult, ToolSchema};

/// Tool trait — every tool must implement this.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;

    /// Generate the OpenAI function-calling schema.
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            schema_type: "function".to_owned(),
            function: FunctionSchema {
                name: self.name().to_owned(),
                description: self.description().to_owned(),
                parameters: self.parameters(),
            },
        }
    }
}

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Panics on duplicate names.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_owned();
        if self.tools.contains_key(&name) {
            panic!("duplicate tool registration: {name}");
        }
        self.tools.insert(name, tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Execute a tool by name.
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
        let tool = self.tools.get(name).ok_or_else(|| AgentCorpError::Tool {
            tool: name.to_owned(),
            message: "tool not found".to_owned(),
        })?;
        tool.execute(args).await
    }

    /// Get OpenAI schemas for all registered tools.
    pub fn get_schemas(&self) -> Vec<ToolSchema> {
        let mut schemas: Vec<_> = self.tools.values().map(|t| t.schema()).collect();
        schemas.sort_by(|a, b| a.function.name.cmp(&b.function.name));
        schemas
    }

    /// Get schemas filtered by disabled list.
    pub fn get_schemas_filtered(&self, disabled: &[String]) -> Vec<ToolSchema> {
        self.tools
            .values()
            .filter(|t| !disabled.contains(&t.name().to_owned()))
            .map(|t| t.schema())
            .collect()
    }

    /// List all tool names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct DummyTool;

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            "dummy"
        }
        fn description(&self) -> &str {
            "A dummy tool"
        }
        fn parameters(&self) -> serde_json::Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult> {
            Ok(ToolResult {
                text: "ok".to_owned(),
                images: vec![],
            })
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));

        assert_eq!(reg.len(), 1);
        assert!(reg.get("dummy").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn schema_generation() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));

        let schemas = reg.get_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].schema_type, "function");
        assert_eq!(schemas[0].function.name, "dummy");
    }

    #[test]
    fn filter_disabled() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));

        let filtered = reg.get_schemas_filtered(&["dummy".to_owned()]);
        assert!(filtered.is_empty());

        let unfiltered = reg.get_schemas_filtered(&[]);
        assert_eq!(unfiltered.len(), 1);
    }

    #[tokio::test]
    async fn execute_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));

        let result = reg.execute("dummy", json!({})).await.unwrap();
        assert_eq!(result.text, "ok");
    }

    #[tokio::test]
    async fn execute_unknown_tool_errors() {
        let reg = ToolRegistry::new();
        let result = reg.execute("unknown", json!({})).await;
        assert!(result.is_err());
    }
}
