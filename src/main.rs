use actix_web::{App, HttpResponse, HttpServer, Responder, get};

#[get("/")]
async fn hellO() -> impl Responder {
    HttpResponse::Ok().body("Hello from actix")
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Server runing at http://localhost:3000");
    HttpServer::new(|| App::new().service(hellO))
        .bind(("127.0.0.1", 3000))?
        .run()
        .await
}
