use crate::model::stt::{STTEvents, StreamingSTTProvider};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};

// "{\"type\":\"Results\",\"channel_index\":[0,1],\"duration\":4.2799997,\"start\":4.33,\"is_final\":true,\"speech_final\":false,\"channel\":{\"alternatives\":[{\"transcript\":\"\",\"confidence\":0.0,\"words\":[]}]},\"metadata\":{\"request_id\":\"01a1db78-471f-7783-8150-1dd0af657fe0\",\"model_info\":{\"name\":\"2-general-nova\",\"version\":\"2024-01-11.36317\",\"arch\":\"nova-2\"},\"model_uuid\":\"1dbdfb4d-85b2-4659-9831-16b3c76229aa\"},\"from_finalize\":false}"
// "{\"type\":\"Results\",\"channel_index\":[0,1],\"duration\":1.9100008,\"start\":8.61,\"is_final\":true,\"speech_final\":true,\"channel\":{\"alternatives\":[{\"transcript\":\"\",\"confidence\":0.0,\"words\":[]}]},\"metadata\":{\"request_id\":\"01a1db78-471f-7783-8150-1dd0af657fe0\",\"model_info\":{\"name\":\"2-general-nova\",\"version\":\"2024-01-11.36317\",\"arch\":\"nova-2\"},\"model_uuid\":\"1dbdfb4d-85b2-4659-9831-16b3c76229aa\"},\"from_finalize\":false}"
// "{\"type\":\"Results\",\"channel_index\":[0,1],\"duration\":2.0,\"start\":10.52,\"is_final\":true,\"speech_final\":true,\"channel\":{\"alternatives\":[{\"transcript\":\"\",\"confidence\":0.0,\"words\":[]}]},\"metadata\":{\"request_id\":\"01a1db78-471f-7783-8150-1dd0af657fe0\",\"model_info\":{\"name\":\"2-general-nova\",\"version\":\"2024-01-11.36317\",\"arch\":\"nova-2\"},\"model_uuid\":\"1dbdfb4d-85b2-4659-9831-16b3c76229aa\"},\"from_finalize\":false}"
// "{\"type\":\"Results\",\"channel_index\":[0,1],\"duration\":0.52999973,\"start\":12.52,\"is_final\":true,\"speech_final\":true,\"channel\":{\"alternatives\":[{\"transcript\":\"\",\"confidence\":0.0,\"words\":[]}]},\"metadata\":{\"request_id\":\"01a1db78-471f-7783-8150-1dd0af657fe0\",\"model_info\":{\"name\":\"2-general-nova\",\"version\":\"2024-01-11.36317\",\"arch\":\"nova-2\"},\"model_uuid\":\"1dbdfb4d-85b2-4659-9831-16b3c76229aa\"},\"from_finalize\":false}"
// "{\"type\":\"Results\",\"channel_index\":[0,1],\"duration\":1.54,\"start\":13.05,\"is_final\":true,\"speech_final\":true,\"channel\":{\"alternatives\":[{\"transcript\":\"\",\"confidence\":0.0,\"words\":[]}]},\"metadata\":{\"request_id\":\"01a1db78-471f-7783-8150-1dd0af657fe0\",\"model_info\":{\"name\":\"2-general-nova\",\"version\":\"2024-01-11.36317\",\"arch\":\"nova-2\"},\"model_uuid\":\"1dbdfb4d-85b2-4659-9831-16b3c76229aa\"},\"from_finalize\":false}"
// "{\"type\":\"Results\",\"channel_index\":[0,1],\"duration\":1.3800001,\"start\":14.59,\"is_final\":true,\"speech_final\":true,\"channel\":{\"alternatives\":[{\"transcript\":\"\",\"confidence\":0.0,\"words\":[]}]},\"metadata\":{\"request_id\":\"01a1db78-471f-7783-8150-1dd0af65

#[derive(Deserialize, Debug)]
pub struct DeepgramResponse {
    #[serde(rename = "type")]
    pub msg_type: Option<String>,
    #[serde(default)]
    pub is_final: bool,
    #[serde(default)]
    pub speech_final: bool,
    pub channel: Option<DeepgramChannel>,
}

#[derive(Deserialize, Debug)]
pub struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Deserialize, Debug)]
struct DeepgramAlternative {
    pub transcript: Option<String>,
    pub confidence: Option<f64>,
}

pub struct DeepgramSTT {
    pub api_key: String,
}

pub enum DeepgramResult {
    Interim(String),
    Final(String),
    Ignore,
}

pub fn parse_deepgram_json(raw_json: &str) -> Result<DeepgramResult, serde_json::Error> {
    let parsed: DeepgramResponse = serde_json::from_str(raw_json)?;

    let x: Option<&str> = parsed.msg_type.as_deref();
    if parsed.msg_type.as_deref() != Some("Results") {
        return Ok(DeepgramResult::Ignore);
    };

    let transcript = parsed
        .channel
        .and_then(|ch: DeepgramChannel| ch.alternatives.into_iter().next())
        .map(|alt| alt.transcript.unwrap_or_default().trim().to_string())
        .unwrap_or_default();

    if transcript.is_empty() {
        return Ok(DeepgramResult::Ignore);
    };

    if parsed.speech_final {
        Ok(DeepgramResult::Final(transcript))
    } else {
        Ok(DeepgramResult::Interim(transcript))
    }
}

#[async_trait::async_trait]
impl StreamingSTTProvider for DeepgramSTT {
    async fn create_stream(
        &self,
    ) -> Result<(Sender<Vec<u8>>, Receiver<STTEvents>), Box<dyn std::error::Error + Send + Sync>>
    {
        let ws_url = "wss://api.deepgram.com/v1/listen?encoding=linear16&sample_rate=16000&channels=1&model=nova-3language=en-IN&smart_format
    =true";
        let (audio_tx, mut audio_rx) = mpsc::channel::<Vec<u8>>(64);
        let (event_tx, event_rx) = mpsc::channel::<STTEvents>(32);
        let mut request = ws_url.into_client_request()?;
        request
            .headers_mut()
            .insert("Authorization", format!("Token {}", self.api_key).parse()?);

        let (ws_stream, _) = match connect_async(request).await {
            Ok(ws) => {
                println!("connected");
                ws
            }
            Err(err) => panic!("err {:?}", err),
        };
        let (mut ws_write, mut ws_read) = ws_stream.split();

        tokio::spawn(async move {
            let send_task = async {
                while let Some(pcm_chunks) = audio_rx.recv().await {
                    let bt = bytes::Bytes::from(pcm_chunks);
                    // sending pcm (voice data ) to datagram using there websocket connection
                    let _ = ws_write.send(Message::Binary(bt)).await;
                }
                let _ = ws_write.send(Message::Close(None)).await;
            };

            let recv_task = async {
                while let Some(Ok(Message::Text(text))) = ws_read.next().await {
                    match parse_deepgram_json(&text) {
                        Ok(DeepgramResult::Final(sentence)) => {
                            let _ = event_tx.send(STTEvents::Final(sentence)).await;
                        }
                        Ok(DeepgramResult::Interim(sentence)) => {
                            let _ = event_tx.send(STTEvents::Interim(sentence)).await;
                        }
                        Ok(DeepgramResult::Ignore) => {}
                        Err(err) => {
                            println!("{:?}", err)
                        }
                    };
                }
            };
            tokio::join!(send_task, recv_task);
        });

        Ok((audio_tx, event_rx))
    }
}
