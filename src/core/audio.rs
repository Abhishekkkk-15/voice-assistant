use std::pin::Pin;

use reqwest::{Client, Error, header::{CONTENT_TYPE, HeaderMap, HeaderValue, IntoHeaderName}};
use bytes;
use futures_util::StreamExt;

use crate::utlis::config::AppConfig;
pub struct Audio{
    req_client:Client
}

impl Audio{
    pub fn new() -> Self{
        Self{
            req_client: Client::new()
        }
    }

    pub async fn stt(&self, text:String)-> Pin<Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>> {
        let config = &AppConfig::global();
        let mut headers = HeaderMap::new();
        headers.insert("Ocp-Apim-Subscription-Key", HeaderValue::from_static(&config.api_key));
        headers.insert("X-Microsoft-OutputFormat", HeaderValue::from_static("audio-16khz-128kbitrate-mono-mp3"));
        headers.insert("User-Agent", HeaderValue::from_static("voice-agent"));
        // Content-Type: application/ssml+xml
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/ssml+xml"));
         let ssml_body = format!("<speak version='1.0' xml:lang='en-US'>                                                                                                                                      
            <voice xml:lang='en-US' name='{}'>{}</voice>                                                                                                                               
        </speak>",                                                                                                                                                                       
        &config.tts_voice, text                                                                                                                                                                  
    );
        let req  = self.req_client.post(config.openai_base_url.clone()).headers(headers).body(ssml_body).send().await;
        req.unwrap().bytes_stream().boxed()
    }

}

//   • Endpoint: https://centralindia.tts.speech.microsoft.com/cognitiveservices/v1                                                                                                          
//   • Header: Ocp-Apim-Subscription-Key: <API_KEY>                                                                                                                                          
//   • Format: X-Microsoft-OutputFormat: audio-16khz-128kbitrate-mono-mp3                                                                                                                    
//   • Body: <speak version='1.0' xml:lang='en-US'><voice name='{tts_voice}'>{text}</voice></speak>    