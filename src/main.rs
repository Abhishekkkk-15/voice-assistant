mod utlis;
mod model;
mod core;

use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, get, web::{self, Data}};
use actix_ws::{Message, MessageStream, Session};
use futures_util::StreamExt;

use tokio::sync::broadcast;
use utlis::session::WSSession;
use core::agent::Agent;

use std::collections::HashMap;
use std::sync::Mutex;
use serde_json;

use crate::model::message::{ClientMessage, WsQuery};

async fn room_handler(mut session: Session, mut msg_stream: MessageStream, data: Data<WSSession>, query: WsQuery) {
    let (room_tx, mut my_rx) = {
        let mut store_guard = data.store.lock().unwrap();
        let tx = store_guard
            .entry(query.room)
            .or_insert_with(|| broadcast::channel::<String>(20).0)
            .clone();

        let rx = tx.subscribe();
        (tx, rx)
    };

    let agent = Agent::new("You are a helpful and concise voice assistant.");

    loop {
        tokio::select! {
            maybe_msg = msg_stream.recv() => {
                match maybe_msg {
                    Some(Ok(msg)) => match msg {
                        Message::Text(text) => {
                            match serde_json::from_str::<ClientMessage>(&text) {
                                Ok(parsed) => {
                                    println!("Received Text JSON: {:?}", parsed);
                                    let _ = room_tx.send(parsed.content);
                                    let _ = session.text("{\"status\":\"received\"}").await;
                                }
                                Err(err) => {
                                    eprintln!("Invalid JSON: {err}");
                                    let err_json = serde_json::json!({ "error": format!("Invalid JSON: {err}") }).to_string();
                                    let _ = session.text(err_json).await;
                                }
                            }
                        }
                        Message::Binary(bin) => {
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
                                let _ = room_tx.send(parsed.content);
                            }
                        }
                        Message::Ping(bytes) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(reason) => {
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
                                    println!("AI Response: {val}");
                                    let res_json = serde_json::json!({
                                        "ai_response": val
                                    }).to_string();
                                    let _ = session.text(res_json).await;

                                    let mut stream = agent.speech_generation(val).await;
                                    let mut chunk_count = 0;
                                    while let Some(chunk) = stream.next().await {
                                        if let Ok(c) = chunk {
                                            chunk_count += 1;
                                            if session.binary(c).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                    println!("🔊 Sent {} TTS audio chunks to client", chunk_count);
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
    // Allow up to 15MB frames so audio recordings never trigger ProtocolError::Overflow
    let msg_stream = msg_stream.max_frame_size(15 * 1024 * 1024);
    println!("WebSocket connected");
    let WsQuery { room, user_id } = query.into_inner();
    let ws_query = WsQuery { room, user_id };
    actix_web::rt::spawn(async move {
        room_handler(session, msg_stream, data, ws_query).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Voice Assistant server running on http://127.0.0.1:8080");
    println!("WebSocket endpoint on ws://127.0.0.1:8080/ws");
    let session: Data<WSSession> = web::Data::new(WSSession {
        store: Mutex::new(HashMap::new()),
    });
    HttpServer::new(move || {
        App::new()
            .app_data(session.clone())
            .service(index)
            .service(ws_handler)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
