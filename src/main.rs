mod utlis;
mod model;
mod core;

use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, get, web::{self, Data}};
use actix_ws::{AggregatedMessage, AggregatedMessageStream, Session};
use futures_util::StreamExt;

use tokio::sync::broadcast;
use utlis::session::WSSession;
use core::agent::{Agent, EXPRESSIVE_VOICE_SYSTEM_PROMPT};

use std::collections::HashMap;
use std::sync::Mutex;
use serde_json;

use crate::model::message::{ClientMessage, WsQuery};

async fn room_handler(mut session: Session, mut msg_stream: AggregatedMessageStream, data: Data<WSSession>, query: WsQuery) {
    let (room_tx, mut my_rx) = {
        let mut store_guard = data.store.lock().unwrap();
        let tx = store_guard
            .entry(query.room)
            .or_insert_with(|| broadcast::channel::<String>(20).0)
            .clone();

        let rx = tx.subscribe();
        (tx, rx)
    };

    let mut current_voice = query.voice.unwrap_or_else(|| "en-US-JennyMultilingualNeural".to_string());
    println!("🎙️ Room handler initialized with voice: {}", current_voice);

    let mut agent = Agent::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);

    loop {
        tokio::select! {
            maybe_msg = msg_stream.recv() => {
                match maybe_msg {
                    Some(Ok(msg)) => match msg {
                        AggregatedMessage::Text(text) => {
                            match serde_json::from_str::<ClientMessage>(&text) {
                                Ok(parsed) => {
                                    println!("Received Text JSON: {:?}", parsed);
                                    if let Some(v) = parsed.voice {
                                        if !v.trim().is_empty() {
                                            println!("🎙️ Selected voice changed to: {}", v);
                                            current_voice = v;
                                        }
                                    }
                                    if !parsed.content.trim().is_empty() {
                                        let _ = room_tx.send(parsed.content);
                                    }
                                    let _ = session.text("{\"status\":\"received\"}").await;
                                }
                                Err(err) => {
                                    eprintln!("Invalid JSON: {err}");
                                    let err_json = serde_json::json!({ "error": format!("Invalid JSON: {err}") }).to_string();
                                    let _ = session.text(err_json).await;
                                }
                            }
                        }
                        AggregatedMessage::Binary(bin) => {
                            // Check if incoming binary data is a WAV audio file from microphone or system audio
                            if bin.starts_with(b"RIFF") {
                                println!("🎤 Received WAV audio payload: {} bytes ({:.1} KB)", bin.len(), bin.len() as f64 / 1024.0);
                                match agent.transcribe(bin.to_vec()).await {
                                    Ok(transcribed_text) => {
                                        println!("🎤 STT Result: '{}'", transcribed_text);
                                        if !transcribed_text.trim().is_empty() {
                                            let res_json = serde_json::json!({
                                                "transcription": transcribed_text
                                            }).to_string();
                                            let _ = session.text(res_json).await;
                                            let _ = room_tx.send(transcribed_text);
                                        } else {
                                            println!("⚠️ STT returned empty text (silence / no speech detected)");
                                            let _ = session.text("{\"status\":\"no_speech_detected\"}").await;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("❌ STT Error: {e}");
                                        let err_json = serde_json::json!({
                                            "error": format!("STT error: {e}")
                                        }).to_string();
                                        let _ = session.text(err_json).await;
                                    }
                                }
                            } else if let Ok(parsed) = serde_json::from_slice::<ClientMessage>(&bin) {
                                println!("Parsed binary JSON: {:?}", parsed);
                                if let Some(v) = parsed.voice {
                                    if !v.trim().is_empty() {
                                        current_voice = v;
                                    }
                                }
                                if !parsed.content.trim().is_empty() {
                                    let _ = room_tx.send(parsed.content);
                                }
                            }
                        }
                        AggregatedMessage::Ping(bytes) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        AggregatedMessage::Close(reason) => {
                            let _ = session.close(reason).await;
                            break;
                        }
                        _ => (),
                    },
                    Some(Err(protocol_err)) => {
                        eprintln!("WebSocket protocol error: {protocol_err:?}");
                        break;
                    }
                    None => {
                        println!("WebSocket stream ended by client");
                        break;
                    }
                }
            }

            broadcast_msg = my_rx.recv() => {
                match broadcast_msg {
                    Ok(msg) => {
                        println!("Processing text for LLM: {msg}");
                        match agent.generation(msg).await {
                            Ok(agent_res) => {
                                if let Some(val) = &agent_res.choices[0].message.content {
                                    println!("AI Raw Response: {val}");

                                    // Parse emotions and construct clean text
                                    let segments = utlis::parser::parse_speech_segments(val);
                                    let clean_text = segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                                    let display_text = if clean_text.trim().is_empty() { val.as_str() } else { clean_text.as_str() };
                                    let emotion = segments.iter().find_map(|s| s.emotion.clone()).unwrap_or_else(|| "neutral".to_string());

                                    let res_json = serde_json::json!({
                                        "ai_response": display_text,
                                        "raw_response": val,
                                        "emotion": emotion
                                    }).to_string();
                                    let _ = session.text(res_json).await;

                                    let mut stream = agent.speech_generation(val, &current_voice).await;
                                    let mut chunk_count = 0;
                                    let mut total_bytes = 0;
                                    while let Some(chunk) = stream.next().await {
                                        if let Ok(c) = chunk {
                                            if !c.is_empty() {
                                                chunk_count += 1;
                                                total_bytes += c.len();
                                                if session.binary(c).await.is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if total_bytes == 0 {
                                        println!("⚠️ TTS returned 0 audio bytes for voice [{}] (language mismatch?)", current_voice);
                                        let warn_json = serde_json::json!({
                                            "status": "tts_empty",
                                            "warning": format!("Voice [{}] cannot synthesize speech for this language/text. Please switch to a Multilingual or Hindi voice.", current_voice)
                                        }).to_string();
                                        let _ = session.text(warn_json).await;
                                    } else {
                                        println!("🔊 Sent {} TTS audio chunks ({:.1} KB) using voice [{}] to client", chunk_count, total_bytes as f64 / 1024.0, current_voice);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("LLM Error: {e}");
                                let err_json = serde_json::json!({
                                    "error": format!("LLM error: {e}")
                                }).to_string();
                                let _ = session.text(err_json).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("Client lagged behind by {n} messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }
}

#[get("/")]
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../index.html"))
}

#[get("/ws")]
async fn ws_handler(req: HttpRequest, stream: web::Payload, data: Data<WSSession>, query: web::Query<WsQuery>) -> Result<HttpResponse, Error> {
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;
    // Allow up to 15MB frames and aggregate continuation frames
    let msg_stream = msg_stream
        .max_frame_size(15 * 1024 * 1024)
        .aggregate_continuations()
        .max_continuation_size(15 * 1024 * 1024);
    println!("WebSocket connected");
    let WsQuery { room, user_id, voice } = query.into_inner();
    let ws_query = WsQuery { room, user_id, voice };
    actix_web::rt::spawn(async move {
        room_handler(session, msg_stream, data, ws_query).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    println!("Voice Assistant server running on http://0.0.0.0:{port}");
    println!("WebSocket endpoint on ws://0.0.0.0:{port}/ws");
    let session: Data<WSSession> = web::Data::new(WSSession {
        store: Mutex::new(HashMap::new()),
    });
    HttpServer::new(move || {
        App::new()
            .app_data(session.clone())
            .service(index)
            .service(ws_handler)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
