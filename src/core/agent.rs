use std::pin::Pin;

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::chat::{ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage, CreateChatCompletionRequestArgs, CreateChatCompletionResponse};

use futures_util::Stream;
use crate::core::audio::Audio;
use crate::utlis::config::AppConfig;
use bytes;

pub struct Agent {
    /// The core OpenAI client used to communicate with all endpoints
    client: Client<OpenAIConfig>,

    // Custom Audio Client
    pub audio_client: Audio,

    /// System prompt setting the assistant's personality
    system_prompt: String,
    /// The sliding window memory of the conversation
    history: Vec<ChatCompletionRequestMessage>,
}

impl Agent {
    pub fn new(system_prompt:&str) -> Self {
        let config = AppConfig::global();
        let mut agent = Agent{ 
            client: Client::with_config(
                OpenAIConfig::new()
                    .with_api_base(config.openai_base_url.clone())
                    .with_api_key(config.openai_api_key.clone())
            ), 
            system_prompt: system_prompt.to_string(), 
            history: Vec::new(),
            audio_client: Audio::new(),
        };

        agent.setup_history();
        agent
        
    }

    fn setup_history(&mut self){

        self.history.push(ChatCompletionRequestSystemMessage::from(self.system_prompt.as_str()).into());
    }

    pub async fn speech_generation(&self, text:&str)-> Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>> {
        self.audio_client.tts(text.to_string()).await
    }

    pub async fn transcribe(&self, audio_data: Vec<u8>) -> Result<String, reqwest::Error> {
        self.audio_client.stt(audio_data).await
    }

    pub async fn generation(&mut self, text:String)->Result<CreateChatCompletionResponse, OpenAIError> {
        &self.history.push(ChatCompletionRequestUserMessage::from(text.clone()).into());
        let config = AppConfig::global();
        let request = CreateChatCompletionRequestArgs::default()
            .model(&config.completion_model)
            .messages(self.history.clone())
            .build()?;

        let res = self.client.chat().create(request).await;
        match res {
            Ok(res) => {
                if let Some(val) = &res.choices[0].message.content {
                    &self.history.push(ChatCompletionRequestAssistantMessage::from(val.clone()).into());
                }
                Ok(res)},
            Err(e) => {
                eprintln!("OpenAI Chat Error: {e:?}");
                Err(e)
            }
        }
    }
}
