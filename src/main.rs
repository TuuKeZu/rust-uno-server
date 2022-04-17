mod ws;
mod lobby;
mod console;
use lobby::Lobby;
mod messages;
mod start_connection;
use start_connection::start_connection as start_connection_route;
use actix::Actor;


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
    .bind("127.0.0.1:8080")?
    .run()
    .await

}