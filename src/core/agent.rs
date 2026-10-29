use std::pin::Pin;

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::chat::{ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage, ChatCompletionTool, CreateChatCompletionRequestArgs, CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FunctionObjectArgs};

use async_openai::types::completions::{CreateCompletionRequestArgs, CreateCompletionResponse};
use futures_util::Stream;
use serde_json::json;
use crate::core::audio::Audio;
use crate::utlis::config::AppConfig;
use bytes;

pub const EXPRESSIVE_VOICE_SYSTEM_PROMPT: &str = r#"You are an expressive, empathetic, and concise voice assistant (Your generated text will be given a TTS model).
Express your emotional tone naturally throughout your response using bracketed emotion tags before sentences or clauses.
Supported emotion tags: [cheerful], [friendly], [empathetic], [excited], [sad], [whispering], [neutral].
You can transition between multiple emotions within a single response when appropriate.
Always keep your answers brief, engaging, and conversational (1 to 3 short sentences max), suitable for spoken audio.
Example:
[empathetic] Oh no, that sounds frustrating! [cheerful] Don't worry though, we can solve this in two quick steps."#;

pub struct Agent {
    /// The core OpenAI client used to communicate with all endpoints
    client: Client<OpenAIConfig>,

    // Custom Audio Client
    pub audio_client: Audio,

    /// System prompt setting the assistant's personality
    system_prompt: String,
    /// The sliding window memory of the conversation
    pub history: Vec<ChatCompletionRequestMessage>,
    pub tools: Option<Vec<i128>>
}

impl Agent {
    pub fn new(system_prompt: &str) -> Self {
        let config = AppConfig::global();
        let mut agent = Agent {
            client: Client::with_config(
                OpenAIConfig::new()
                    .with_api_base(config.openai_base_url.clone())
                    .with_api_key(config.openai_api_key.clone())
            ),
            system_prompt: system_prompt.to_string(),
            history: Vec::new(),
            audio_client: Audio::new(),
            tools:None
        };

        agent.setup_history();
        agent
    }

    fn setup_history(&mut self) {
        self.history.push(ChatCompletionRequestSystemMessage::from(self.system_prompt.as_str()).into());
    }

    pub async fn speech_generation(&self, text: &str, voice: &str) -> Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>> {
        self.audio_client.tts(text, voice).await
    }

    pub async fn transcribe(&self, audio_data: Vec<u8>) -> Result<String, reqwest::Error> {
        self.audio_client.stt(audio_data).await
    }
    // one shot completion
    pub async fn generation(&mut self, text: String) -> Result<CreateChatCompletionResponse, OpenAIError> {
        self.history.push(ChatCompletionRequestUserMessage::from(text).into());
        let config = AppConfig::global();
        let request = CreateChatCompletionRequestArgs::default()
            .model(&config.completion_model)
            .messages(self.history.clone())
            .tools(ChatCompletionTool {
            function: FunctionObjectArgs::default()
                .name("get_current_weather")
                .description("Get the current weather in a given location")
                .parameters(json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city and state, e.g. San Francisco, CA",
                        },
                        "unit": { "type": "string", "enum": ["celsius", "fahrenheit"] },
                    },
                    "required": ["location", "unit"],
                    "additionalProperties": false
                }))
                .strict(true)
                .build()?,
        })
            .build()?;

        let res = self.client.chat().create(request).await;
        match res {
            Ok(res) => {
                if let Some(val) = &res.choices[0].message.content {
                    self.history.push(ChatCompletionRequestAssistantMessage::from(val.clone()).into());
                }
                Ok(res)
            }
            Err(e) => {
                eprintln!("OpenAI Chat Error: {e:?}");
                Err(e)
            }
        }
    }

    // stream 
    pub async fn generation_stream(&mut self, text: String) -> Result<Pin<Box<dyn Stream<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>> + Send>>, OpenAIError> {
        self.history.push(ChatCompletionRequestUserMessage::from(text).into());
        let config = AppConfig::global();
        let result = async {
            let request = CreateChatCompletionRequestArgs::default()
                .model(&config.completion_model)
                .messages(self.history.clone())
                .stream(true)
                .build()?;

            self.client.chat().create_stream(request).await
        }
        .await;

        match result {
            Ok(res) => Ok(Box::pin(res)),
            Err(err) => Err(err),
        }
    }

    pub fn record_assistant_response(&mut self, text: String) {
        self.history.push(ChatCompletionRequestAssistantMessage::from(text).into());
    }
}

// async fn sd() -> Result<(), Box<dyn std::error::Error>> {
//     let client = Client::new();

//     let request = CreateCompletionRequestArgs::default()
//         .model("gpt-3.5-turbo-instruct")
//         .n(1)
//         .prompt("Tell me a bedtime story about Optimus Prime and Bumblebee")
//         .stream(true)
//         .max_tokens(1024_u32)
//         .build()?;

//     let mut stream = client.completions().create_stream(request).await?;
// }