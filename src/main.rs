mod console;
mod errors;
mod lobby;
mod packets;
mod ws;
use lobby::Lobby;
mod game;
mod messages;
mod start_connection;
use actix::Actor;
use start_connection::start_connection as start_connection_route;

use actix_web::{App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let chat_server = Lobby::default().start();

    println!("Server Started!");

    HttpServer::new(move || {
        App::new()
            .service(start_connection_route)
            .data(chat_server.clone())
    })
    .bind("127.0.0.1:8090")?
    .run()
    .await
}
