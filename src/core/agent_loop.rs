use std::{collections::HashMap};

use async_openai::types::{chat::{ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, ChatCompletionRequestMessage, ChatCompletionRequestUserMessage, FunctionCall}, chatkit::UserMessageContent};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{sync::mpsc, task::JoinHandle};
// use tokio::sync::mpsc::{channel};
use crate::{core::{agent::Agent, messages::MessagesRegistry}, model::events::AssistantEvent};
use rand::{rng, seq::IndexedRandom};

#[derive(Debug,Clone)]
struct PendingToolCall {
        id: String,
        name: String,
        arguments: String,
}

impl From<PendingToolCall> for ChatCompletionMessageToolCalls {
        fn from(tool: PendingToolCall) -> Self {
            ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                id: tool.id,
                function: FunctionCall {
                    name: tool.name,
                    arguments: tool.arguments,
                },
            })
        }
    }


pub async fn agent_loop(mut agent:Agent, user_input:String,outbound_tx: mpsc::Sender<AssistantEvent>, mut memory:MessagesRegistry) -> Result<(),  Box<dyn std::error::Error>>{


    let should_break = false;
    memory.user(user_input.clone());
    
    loop {
        let mut llm_stream = match agent.generation_stream_for_agent( memory.get_messages()).await{
             Ok(stream) => stream,
        Err(err) => {
            eprintln!("LLM Stream Error: {err:?}");
            let _ = outbound_tx.send(AssistantEvent::Error(err.to_string())).await;
            return Ok(());
        }
        };

        let mut tool_calls_vec:Vec<PendingToolCall> = Vec::new();
        let mut assistant_text: String = String::new();
        while let Some(stream_res) = llm_stream.next().await{
            match stream_res {
                Ok(chunk) =>{
                    for choice in chunk.choices{
                        if let Some(content) = choice.delta.content{
                                // outbound_tx.send(content)
                                println!("{content}");
                                assistant_text += &content;
                        }
                        if let Some(tool_chunk) = choice.delta.tool_calls{
                            for tool in tool_chunk{
                                let idx  = tool.index as usize;
                                if idx >= tool_calls_vec.len(){
                                    tool_calls_vec.push(PendingToolCall { id: tool.id.unwrap().clone(), 
                                    name:tool.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(),
                                    arguments: tool.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default() });
                                }else{
                                    let in_vec = &tool_calls_vec[idx];
                                    if let Some(tool_id) = tool.id{
                                        tool_calls_vec[idx].id += &tool_id;
                                    }else if let Some(tool_fn) = &tool.function{
                                            if let Some(fn_name) = &tool_fn.name{
                                        tool_calls_vec[idx].name += &fn_name;
                                            }
                                            if let Some(fn_args) = &tool_fn.arguments{
                                        tool_calls_vec[idx].arguments += &fn_args;
                                            }
                                    }
                                    // tool_calls_vec.push(value);
                                }
                            };
                        }
                    }
                },
                Err(err) =>{}
            }
        }

        if tool_calls_vec.is_empty() {
        if !assistant_text.trim().is_empty() {
            memory.assistant(assistant_text);
        }
        break;
    }

        // let res_content = String::from(&res.choices[0].message.clone().content.unwrap());
        let open_ai_tools: Vec<ChatCompletionMessageToolCalls> = tool_calls_vec
        .iter()
        .cloned()
        .map(Into::into)
        .collect();
    let text_opt = if assistant_text.trim().is_empty() { None } else { Some(assistant_text) };
        memory.assistant_with_tool_calls(text_opt,&open_ai_tools);
      

            let mut handles= Vec::new();

            for tool_call in tool_calls_vec{
                //   if let ChatCompletionMessageToolCalls::Function(tool_call) = tool_call_enum {
                        let name = tool_call.name.clone();
                        let args = tool_call.arguments.clone();
                        let tool_call_clone = tool_call.clone();
                            println!("\n🔧 Calling tool: {} with args: {}", tool_call.name, tool_call.arguments);
                        let handle: tokio::task::JoinHandle<Value> = tokio::spawn(async move {call_fn(&name, &args).await.unwrap_or_default()});
                        handles.push((handle, tool_call));
                //   };

            }
            
            let mut function_responses = Vec::new();
            
            for (handle, tool_call_clone) in handles{
                if let Ok(response_content) = handle.await {
                     println!("📊 Tool result: {}\n", response_content);
                function_responses.push((tool_call_clone, response_content));
            }
            }
            for f in function_responses.iter() {
                memory.tool_response(&f.0.id, f.1.to_string());
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



