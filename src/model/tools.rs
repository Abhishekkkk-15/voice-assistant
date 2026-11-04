// use as

use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync{
     fn name(&self) -> &str;   
     fn description(&self) -> &str;
     fn parameter_schema(&self) -> Value;
     async fn run(&self, args:&str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;     
}