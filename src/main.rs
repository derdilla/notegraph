use actix_web::{App, HttpServer};
use clap::Parser;
use config::Config;
use models::Model;
use server::start_server;

mod config;
mod models;
mod server;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Config::parse();
    let model = Model::read(&config.input_dir).unwrap();

    //println!("{:?}", model);

    start_server(model).await
}
