mod console;
mod errors;
mod lobby;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
mod packets;
mod ws;
use lobby::Lobby;
mod game;
mod messages;
mod start_connection;
use actix::Actor;
use actix_web::{App, HttpServer};
use clap::{Arg, Command};
use serde_derive::Deserialize;
use start_connection::start_connection as start_connection_route;
use std::fs;
use toml;

#[derive(Deserialize)]
struct Config {
    listen_addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let matches = Command::new("rust-uno-server")
        .arg(
            Arg::new("config")
                .short('c')
                .value_name("CONFIG FILE")
                .help("Configuration file")
                .takes_value(true),
        )
        .get_matches();

    let config_file = matches.value_of("config");
    let config: Config = config_file
        .map(|config_file| {
            toml::from_str(
                fs::read_to_string(config_file)
                    .expect("file not found")
                    .as_str(),
            )
            .expect("invalid config")
        })
        .unwrap_or_default();

    let chat_server = Lobby::default().start();

    println!("Server Started at {}", config.listen_addr);

    HttpServer::new(move || {
        App::new()
            .service(start_connection_route)
            .data(chat_server.clone())
    })
    .bind(config.listen_addr)?
    .run()
    .await
}
