use std::pin::Pin;

use async_openai::Client;
use async_openai::config::{AzureConfig, OpenAIConfig };
use async_openai::error::OpenAIError;
use async_openai::types::chat::{ChatCompletionRequestAssistantMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequest, CreateChatCompletionRequestArgs, CreateChatCompletionResponse, CreateChatCompletionStreamResponse};
use async_openai::types::completions::{CreateCompletionRequest};

use async_openai::types::realtime::{Error, SystemMessageContent};
use async_openai::types::stream::StreamResponse;
use futures_util::{Stream, StreamExt};
use crate::core::audio::Audio;
use crate::utlis::config::AppConfig;
use bytes;

pub struct Agent {
    /// The core OpenAI client used to communicate with all endpoints
    client: Client<OpenAIConfig>,

    // Custom Audio Client
    audio_client:Audio,

    /// System prompt setting the assistant's personality
    system_prompt: String,
    /// The sliding window memory of the conversation
    history: Vec<CreateCompletionRequest>,
}


impl Agent{
    pub fn new(system_prompt:&str) -> Self {
        let config = &AppConfig::global();
   
        Self { client: Client::with_config(OpenAIConfig::new().with_api_base(config.openai_base_url.clone()).with_api_key(config.openai_api_key.clone())), system_prompt: system_prompt.to_string(), history: Vec::new(),audio_client:Audio::new() }
    }

    pub async fn speech_generation(&self, text:&str)-> Pin<Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>{
        let stream = self.audio_client.tts(text.to_string()).await;
        // while let Some(chunks) = stream.await.next().await{
        //     Ok(chunks)
        // }
        return stream

    }

    pub async fn generation(&self, text:String)->Result<CreateChatCompletionResponse, OpenAIError>{

    //    let request = CreateChatCompletionRequestArgs::default().model("gpt-5.6-luna")
    //    .messages([
    //         ChatCompletionRequestSystemMessageArgs::default()
    //             .content(self.system_prompt.clone())
    //             .build().unwrap()
    //             .into()
    //     ])
    //    .build().unwrap();

        let reques2t = CreateChatCompletionRequestArgs::default()
        .model("gpt-5.6-luna")
        .messages([
            // Can also use ChatCompletionRequest<Role>MessageArgs for builder pattern
            ChatCompletionRequestSystemMessage::from(self.system_prompt.clone()).into(),
            ChatCompletionRequestUserMessage::from(text).into()
        ])
        .build()?;

        let  stream = self.client.chat().create(reques2t).await;
       match stream{
        Ok(res) => Ok(res),
        Err(e) => {
            println!("{:?}",e);
            Err(e)}
       }
    }

}

                                                                                                                                                              
    