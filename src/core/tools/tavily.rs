use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::{model::tools::Tool, utlis::config::AppConfig};

#[derive(Deserialize)]
pub struct TavilyArgs {
    query: String,
}

pub struct TavilySearchTool;

#[async_trait]
impl Tool for TavilySearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the live internet for recent news, events, real-time facts, people, and general web information. Use this whenever the user asks about current events, facts, or information you are uncertain about."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to look up on the web"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let config = AppConfig::global();
        let api_key = &config.tavily_api_key;

        if api_key.is_empty() {
            return Err("TAVILY_API_KEY is not configured in .env. Please set TAVILY_API_KEY to enable web search.".into());
        }

        let arguments: TavilyArgs = serde_json::from_str(args)?;
        let query = arguments.query.trim();

        let client = reqwest::Client::new();
        let payload = json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": true,
            "max_results": 3
        });

        let response = client
            .post("https://api.tavily.com/search")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Tavily search API error: {error_text}").into());
        }

        let data: Value = response.json().await?;

        let answer = data.get("answer")
            .and_then(|a| a.as_str())
            .unwrap_or("");

        let empty_vec = vec![];
        let results = data.get("results")
            .and_then(|r| r.as_array())
            .unwrap_or(&empty_vec);

        let formatted_results: Vec<Value> = results
            .iter()
            .take(3)
            .map(|item| {
                json!({
                    "title": item["title"].as_str().unwrap_or(""),
                    "snippet": item["content"].as_str().unwrap_or(""),
                    "url": item["url"].as_str().unwrap_or("")
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "direct_answer": if answer.is_empty() { Value::Null } else { json!(answer) },
            "results": formatted_results
        }))
    }
}
