use crate::model::stt::{STTEvents,StreamingSTTProvider};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{Sender,Receiver};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use serde::Deserialize;


#[derive(Deserialize)]
pub struct DeepgramResponse{
    #[serde(rename = "type")]
    pub msg_type:Option<String>,
    #[serde(default)]
    pub is_final:bool,
    #[serde(default)]
    pub speech_final:bool,
    pub channel:Option<DeepgramChannel>
}

#[derive(Deserialize)]
pub struct DeepgramChannel{
    alternatives: Vec<DeepgramAlternative>
}

#[derive(Deserialize)]
struct DeepgramAlternative{
    pub transcript:String,
    pub confidence:Option<f64>
}


pub struct DeepgramSTT{
    api_key:String
}

pub enum DeepgramResult{
    Interim(String),
    Final(String),
    Ignore
}

pub fn parse_deepgram_json(raw_json:&str) -> Result<DeepgramResult, serde_json::Error>{
    let parsed: DeepgramResponse= serde_json::from_str(raw_json)?;
    let x: Option<&str>=  parsed.msg_type.as_deref();
    if parsed.msg_type.as_deref() != Some("Results"){
        return Ok(DeepgramResult::Ignore);
    };

    let transcript = parsed.channel.and_then(|ch: DeepgramChannel| ch.alternatives.into_iter().next()).map(|alt| alt.transcript.trim().to_string()).unwrap_or_default();

    if transcript.is_empty(){
        return Ok(DeepgramResult::Ignore);
    };

    if parsed.is_final{
        Ok(DeepgramResult::Final(transcript))
    }else{
        Ok(DeepgramResult::Interim(transcript))
    }
}

#[async_trait::async_trait]
impl StreamingSTTProvider for   DeepgramSTT {
    async fn create_stream(&self) ->  Result<(Sender<Vec<u8>>, Receiver<STTEvents>), Box<dyn std::error::Error + Send + Sync>> {
        let ws_url = "wss://api.deepgram.com/v1/listen";
        let (audio_tx, mut audio_rx) = mpsc::channel::<Vec<u8>>(64);
        let (event_tx, event_rx) = mpsc::channel::<STTEvents>(32);
    let mut request = ws_url.into_client_request()?;
    request.headers_mut().insert(
        "Authorization",
        format!("Token {}", self.api_key).parse()?,
    );

        let (ws_stream,_) = connect_async(request).await?;
        let (mut ws_write, mut ws_read) = ws_stream.split();

        tokio::spawn(async move {
            let send_task = async{
                while let Some(pcm_chunks) = audio_rx.recv().await{
                    let bt = bytes::Bytes::from(pcm_chunks);
                    // sending pcm (voice data ) to datagram using there websocket connection
                    let _ =ws_write.send(Message::Binary(bt)).await;
                }
                let _ = ws_write.send(Message::Close(None)).await;
            };

            let recv_task = async{
                while let Some(Ok(Message::Text(text))) = ws_read.next().await{
                    match parse_deepgram_json(&text){
                        Ok(DeepgramResult::Final(sentence)) =>{
                            let _ = event_tx.send(STTEvents::Final(sentence)).await;
                        },
                        Ok(DeepgramResult::Interim(sentence)) => {
                            let _ = event_tx.send(STTEvents::Interim(sentence)).await;
                        },
                        Ok(DeepgramResult::Ignore ) => {},
                        Err(err)=>{
                            println!("{:?}",err)
                        }
                    };
                }
            };
            tokio::join!(send_task,
                recv_task);
        });




    Ok((audio_tx, event_rx))

    }


}