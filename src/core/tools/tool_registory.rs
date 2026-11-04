use std::{collections::HashMap, sync::Arc};

use async_openai::types::chat::{ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, FunctionObject};
use serde_json::Value;

use crate::{core::tools::tool_registory, model::tools::Tool};

pub struct ToolRegistory{
    tools: HashMap<String, Arc<dyn Tool>>
}

// impl From<ToolRegistory> for ChatCompletionMessageToolCalls {
//     fn from(value: ToolRegistory) -> Self {
//         ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall{
            
//         })
//     }
// }

impl ToolRegistory{
    pub fn new() -> Self{
        Self {
            tools: HashMap::new()
        }
    }

    pub fn register(&mut self, tool:Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn execute(&self,tool_name:&str, args: &str)->Result<Value, String>{
        let tool = self.tools.get(tool_name).ok_or_else(|| format!("Tool '{tool_name}' not found"))?;
        tool.run(args).await.map_err(|e| e.to_string())
    }

     pub fn to_openai_tools(&self) -> Vec<async_openai::types::chat::ChatCompletionTool> {
        // Formatted directly for agent.generation_stream_for_agent(...)

        let mut openai_tool_schema:Vec<async_openai::types::chat::ChatCompletionTool> = Vec::new();

        for tool in self.tools.iter()  {
            let tool_schema = tool.1;
            openai_tool_schema.push(
                async_openai::types::chat::ChatCompletionTool { function: FunctionObject{
                        name:tool_schema.name().to_string(),
                        description: Some(tool_schema.description().to_string()),
                        parameters:Some(tool_schema.parameter_schema()),
                        strict:Some(true)
                } }
            );
            
        };
        // self.tools.iter().clone().map(|f| f.1.as_ref())
        return openai_tool_schema
    }

}