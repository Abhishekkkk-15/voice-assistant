mod utlis;
mod model;
mod core;

use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, Responder, get, web::{self, Data}};
use actix_ws::{Message, MessageStream, Session};
use futures_util::StreamExt;

use tokio::sync::broadcast;
use utlis::{session::{WSSession}};
use core::agent::{Agent};

use std::{collections::HashMap, sync::{ Mutex, mpsc::Sender}};
use std::sync::Arc;
use serde_json;


use crate::model::message::{ClientMessage, WsQuery};

async fn room_handler(mut session:Session,mut msg_stream: MessageStream, data:Data<WSSession>,query:WsQuery){
 
    let (room_tx, mut my_rx) = {
    let mut store_guard = data.store.lock().unwrap();
    let tx = store_guard
        .entry(query.room)
        .or_insert_with(|| broadcast::channel::<String>(20).0)
        .clone(); 

    let rx = tx.subscribe();
    (tx, rx)
    }; 
                        let agent = Agent::new("U are a help full assistent");

 loop { 
        tokio::select! {
            maybe_msg = msg_stream.recv() => {
                match maybe_msg {
                    Some(Ok(msg)) => match msg {
                        Message::Text(text) => {
                            match serde_json::from_str::<ClientMessage>(&text) {
                                Ok(parsed) => {
                                    println!("Received JSON: {:?}", parsed);

                                    let _ = room_tx.send(parsed.content);
                                    
                                    let _ = session.text("{\"status\":\"received\"}").await;
                                    
                                }
                                Err(err) => {
                                    eprintln!("Invalid JSON: {err}");
                                    let _ = session.text(format!("{{\"error\":\"{err}\"}}")).await;
                                }
                            }
                        }
                        Message::Binary(bin) => {
                            if let Ok(parsed) = serde_json::from_slice::<ClientMessage>(&bin) {
                                println!("Parsed binary JSON: {:?}", parsed);
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
                    _ => break,
                }
            }

            broadcast_msg = my_rx.recv() => {
                match broadcast_msg {
                    Ok(msg) => {
                        let mut stream = agent.speech_generation(&msg).await;
                        while let Some(chunk) = stream.next().await{
                            
                            if session.binary(chunk.unwrap()).await.is_err(){
                                break;
                            };
                        };
                     
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

#[get("/ws")]
async fn ws_handler(req: HttpRequest, stream: web::Payload, data:Data<WSSession>,query:web::Query<WsQuery>) -> Result<HttpResponse, Error> {
    
    let (response,  session, msg_stream) = actix_ws::handle(&req, stream)?;
    println!("Got here");
    let WsQuery { room, user_id } = query.into_inner();
    let ws_query = WsQuery { room, user_id };
    actix_web::rt::spawn(async move {
        room_handler(session, msg_stream, data,ws_query).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("WebSocket server running on 127.0.0.1:8080/ws");
    let session :Data<WSSession> =web::Data::new( WSSession{
        store:  Mutex::new(HashMap::new())
    });
    HttpServer::new(move || App::new()
    .app_data(session.clone())
    .service(ws_handler))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}