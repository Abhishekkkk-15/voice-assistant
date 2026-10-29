use std::{collections::HashMap};

use async_openai::types::{chat::{ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, ChatCompletionRequestMessage, ChatCompletionRequestUserMessage}, chatkit::UserMessageContent};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
// use tokio::sync::mpsc::{channel};
use crate::core::agent::Agent;
use rand::{rng, seq::IndexedRandom};

pub async fn agent_loop(mut agent:Agent, user_input:String) -> Result<(),  Box<dyn std::error::Error>>{


    let should_break = false;

    loop {
        let loop_res = agent.generation(user_input.clone()).await?;
        let tool_call = loop_res.choices.first().ok_or("no_choics")?.message.clone();

        if tool_call.tool_calls.is_none(){
            
            break;
        }

        if let Some(tool_calls) = tool_call.tool_calls{
            let mut handles:Vec<(JoinHandle<Value>,ChatCompletionMessageToolCall)> = Vec::new();

            for tool_call_enum in tool_calls{
                  if let ChatCompletionMessageToolCalls::Function(tool_call) = tool_call_enum {
                        let name = tool_call.function.name.clone();
                        let args = tool_call.function.arguments.clone();
                        let tool_call_clone = tool_call.clone();

                        let handle: tokio::task::JoinHandle<Value> = tokio::spawn(async move {call_fn(&name, &args).await.unwrap_or_default()});
                        handles.push((handle, tool_call_clone));
                  };

            }

            let mut function_responses = Vec::new();

            for (handle, tool_call_clone) in handles{
                 if let Ok(response_content) = handle.await {
                function_responses.push((tool_call_clone, response_content));
            }
            }

        }


                    

    }

    Ok(())

}


async fn call_fn(name: &str, args: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut available_functions: HashMap<&str, fn(&str, &str) -> serde_json::Value> =
        HashMap::new();
    available_functions.insert("get_current_weather", get_current_weather);

    let function_args: serde_json::Value = args.parse().unwrap();

    let location = function_args["location"].as_str().unwrap();
    let unit = function_args["unit"].as_str().unwrap_or("fahrenheit");
    let function = available_functions.get(name).unwrap();
    let function_response = function(location, unit);
    Ok(function_response)
}

fn get_current_weather(location: &str, unit: &str) -> serde_json::Value {
    
     let mut rng = rng();
    let temperature: i32 = rand::random_range(20..=55);

    let forecasts = [
        "sunny", "cloudy", "overcast", "rainy", "windy", "foggy", "snowy",
    ];

    let forecast = forecasts.choose(&mut rng).unwrap_or(&"sunny");

    let weather_info = json!({
        "location": location,
        "temperature": temperature.to_string(),
        "unit": unit,
        "forecast": forecast
    });

    weather_info
}



