mod utlis;
mod model;
mod core;
mod transports;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, get, web};

use crate::{model::message::WsQuery, transports::ws::ws_transport};

#[get("/")]
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../index.html"))
}

#[get("/ws")]
async fn ws_handler(req: HttpRequest, stream: web::Payload, query: web::Query<WsQuery>) -> Result<HttpResponse, Error> {
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;
    // Allow up to 15MB frames and aggregate continuation frames
    let msg_stream = msg_stream
        .max_frame_size(15 * 1024 * 1024)
        .aggregate_continuations()
        .max_continuation_size(15 * 1024 * 1024);
    println!("WebSocket connected");
    let ws_query = query.into_inner();
    actix_web::rt::spawn(async move {
        ws_transport(session, msg_stream, ws_query).await;
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

    HttpServer::new(move || {
        App::new()
            .service(index)
            .service(ws_handler)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
