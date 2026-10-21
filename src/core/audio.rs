use std::pin::Pin;

use reqwest::{Client, Error, header::{CONTENT_TYPE, HeaderMap, HeaderValue}};
use bytes;
use futures_util::StreamExt;
use serde::Deserialize;

use crate::utlis::config::AppConfig;

pub struct Audio {
    req_client: Client,
}

#[derive(Deserialize, Debug)]
struct SttResponse {
    #[serde(rename = "RecognitionStatus")]
    recognition_status: String,
    #[serde(rename = "DisplayText")]
    display_text: Option<String>,
}

impl Audio {
    pub fn new() -> Self {
        Self {
            req_client: Client::new(),
        }
    }

    pub async fn tts(&self, text: String) -> Pin<Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>> {
        let config = AppConfig::global();
        let mut headers = HeaderMap::new();
        headers.insert("Ocp-Apim-Subscription-Key", HeaderValue::from_str(&config.api_key).unwrap());
        headers.insert("X-Microsoft-OutputFormat", HeaderValue::from_static("audio-16khz-128kbitrate-mono-mp3"));
        headers.insert("User-Agent", HeaderValue::from_static("voice-agent"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/ssml+xml"));

        let ssml_body = format!(
            "<speak version='1.0' xml:lang='en-US'>\
                <voice xml:lang='en-US' name='{}'>{}</voice>\
            </speak>",
            &config.tts_voice, text
        );

        let req = self.req_client
            .post(config.azure_audio_url.clone())
            .headers(headers)
            .body(ssml_body)
            .send()
            .await;

        req.unwrap().bytes_stream().boxed()
    }

    pub async fn stt(&self, audio_data: Vec<u8>) -> Result<String, Error> {
        let config = AppConfig::global();
        let mut headers = HeaderMap::new();

        // 1. Azure Speech Subscription key
        headers.insert(
            "Ocp-Apim-Subscription-Key",
            HeaderValue::from_str(&config.api_key).expect("invalid characters in API key"),
        );

        // 2. Audio format content-type (WAV PCM 16kHz mono)
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("audio/wav; codecs=audio/pcm; samplerate=16000"),
        );
        headers.insert("User-Agent", HeaderValue::from_static("voice-agent"));
        headers.insert("Accept", HeaderValue::from_static("application/json"));

        // 3. Azure STT REST endpoint (using centralindia where your speech service is provisioned)
        let stt_url = "https://centralindia.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1?language=en-US";

        // 4. Send raw audio bytes in POST body
        let res = self
            .req_client
            .post(stt_url)
            .headers(headers)
            .body(audio_data)
            .send()
            .await?
            .error_for_status()?;

        // 5. Parse JSON response
        let stt_result: SttResponse = res.json().await?;

        if stt_result.recognition_status == "Success" {
            Ok(stt_result.display_text.unwrap_or_default())
        } else {
            Ok(String::new())
        }
    }
}
