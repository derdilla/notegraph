use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_files::NamedFile;
use serde::Serialize;

use crate::models::Model;

#[get("/")]
async fn index() -> impl Responder {
    // Serve the static index.html from project root
    let content = include_str!("../index.html");
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content)
}

#[get("/api/nodes")]
async fn get_nodes(data: web::Data<Model>) -> impl Responder {
    HttpResponse::Ok()
        .json(data.get_nodes())
}

#[get("/api/edges")]
async fn get_edges() -> impl Responder {
    // TODO: Implement actual edge retrieval
    let data: Vec<Vec<String>> = vec![];
    HttpResponse::Ok()
        .json(data)
}

pub async fn start_server(data: Model) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .service(index)
            .service(get_nodes)
            .service(get_edges)
    })
    .bind(("127.0.0.1", 8080))?  // Bind to localhost:8080
    .run()
    .await
}