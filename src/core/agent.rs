use std::pin::Pin;

use async_openai::Client;
use async_openai::config::{AzureConfig };
use async_openai::types::completions::CreateCompletionRequest;
use futures_util::StreamExt;
use crate::core::audio::Audio;
use crate::utlis::config::AppConfig;
use bytes;

pub struct Agent {
    /// The core OpenAI client used to communicate with all endpoints
    client: Client<AzureConfig>,

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
   
        Self { client: Client::with_config(AzureConfig::new().with_api_base(config.openai_base_url.clone()).with_api_key(config.api_key.clone())), system_prompt: system_prompt.to_string(), history: Vec::new(),audio_client:Audio::new() }
    }

    pub async fn speech_generation(&self, text:&str)-> Pin<Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>{
        let stream = self.audio_client.stt(text.to_string()).await;
        // while let Some(chunks) = stream.await.next().await{
        //     Ok(chunks)
        // }
        return stream

    }

}

                                                                                                                                                              
    