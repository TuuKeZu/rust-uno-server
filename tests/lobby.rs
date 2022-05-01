use actix::Actor;
use actix_web::rt::net::TcpStream;
use colored::Colorize;
use futures_util::stream::SplitSink;
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde_json::Value;
use std::net::TcpListener;
use uno_server::lobby::Lobby;
use uno_server::start_connection::start_connection as start_connection_route;
use uuid::Uuid;

use actix_web::{App, HttpServer};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, tungstenite::Error, MaybeTlsStream,
    WebSocketStream,
};

#[derive(Debug, Clone)]
struct Response {
    pub r#type: String,
}

impl Response {
    pub fn new(r#type: String) -> Response {
        Response { r#type }
    }
}

fn start_server() -> (u16, actix_web::rt::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = actix_rt::spawn(async {
        HttpServer::new(|| {
            let chat_server = Lobby::default().start();

            App::new().service(start_connection_route).data(chat_server)
        })
        .listen(listener)
        .unwrap()
        .run()
        .await
        .unwrap();
    });

    dbg!(port);

    (port, handle)
}

fn log_message(data: Option<Result<Message, Error>>) -> Option<Response> {
    let v: serde_json::Result<Value> =
        serde_json::from_str(data.unwrap().unwrap().to_text().unwrap());

    match v {
        Ok(result) => {
            let r#type = &result["type"];

            if r#type.is_null() {
                return None;
            };
            /*
            if result["type"] == "ERROR" {
                println!("{}", format!("{:#?}", result).red());
            } else {
                println!("{}", format!("{:#?}", result).yellow());
            }
            */
            Some(Response::new(r#type.as_str().unwrap().to_string()))
        }
        Err(e) => {
            dbg!(format!("Failed to convert response to json: {}", e).red());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_rt::test]
    async fn connection_works() -> Result<(), Box<dyn std::error::Error>> {
        let (port, server_handle) = start_server();

        // cargo test -- --nocapture
        let client_handle = actix_web::rt::spawn(async move {
            let (ws_stream, _) = connect_async(format!("ws://127.0.0.1:{port}/{}", Uuid::new_v4()))
                .await
                .unwrap();

            let (mut write, mut read) = ws_stream.split();
        });

        client_handle.await?;
        drop(server_handle);
        Ok(())
    }

    #[actix_rt::test]
    async fn game_start() -> Result<(), Box<dyn std::error::Error>> {
        let (port, server_handle) = start_server();

        // cargo test -- --nocapture
        let client_handle = actix_web::rt::spawn(async move {
            // Initialize client
            let (player_1, _) = connect_async(format!("ws://127.0.0.1:{port}/{}", Uuid::new_v4()))
                .await
                .unwrap();

            let (mut p_write_1, mut p_read_1) = player_1.split();

            // Initialize client
            let (player_2, _) = connect_async(format!("ws://127.0.0.1:{port}/{}", Uuid::new_v4()))
                .await
                .unwrap();

            let (mut p_write_2, mut p_read_2) = player_2.split();

            // Send Request - Register
            p_write_1
                .send(Message::Text(
                    r#"{"type": "REGISTER", "username": "test_1" }"#.to_string(),
                ))
                .await
                .unwrap();

            // Send Request - Register
            p_write_2
                .send(Message::Text(
                    r#"{"type": "REGISTER", "username": "test_2"}"#.to_string(),
                ))
                .await
                .unwrap();

            // Send Request - Register a second time
            p_write_2
                .send(Message::Text(
                    r#"{"type": "REGISTER", "username": "test_2"}"#.to_string(),
                ))
                .await
                .unwrap();

            // Send Request - Start the game without permission
            p_write_2
                .send(Message::Text(
                    r#"{"type": "START-GAME","options": "None"}"#.to_string(),
                ))
                .await
                .unwrap();

            let mut responses: Vec<Response> = Vec::new();

            // Read player 2's responses
            while let Some(message) = p_read_2.next().now_or_never() {
                if let Some(r) = log_message(message) {
                    responses.push(r);
                }
            }

            assert!(
                responses.iter().any(|res| res.r#type == "ERROR"),
                "Two instances were created withour emiting an error and (or) game was started without permission"
            );

            // reset responses
            responses.clear();

            // Send Request - Start game with permissio
            p_write_1
                .send(Message::Text(
                    r#"{"type": "START-GAME","options": "None"}"#.to_string(),
                ))
                .await
                .unwrap();

            // Read player 1's responses
            while let Some(message) = p_read_1.next().now_or_never() {
                if let Some(r) = log_message(message) {
                    responses.push(r);
                }
            }

            // dbg!(&responses);

            assert!(
                !responses.iter().any(|res| res.r#type == "ERROR"),
                "Error was emitted when trying to start the game"
            );

            assert!(
                !responses
                    .iter()
                    .any(|res| res.r#type == "STATUS-UPDATE-PRIVATE"),
                "Failed to receive your initial cards"
            );

            assert!(
                !responses
                    .iter()
                    .any(|res| res.r#type == "STATUS-UPDATE-PRIVATE"),
                "Failed to receive the public cards"
            );
        });

        client_handle.await?;
        drop(server_handle);
        Ok(())
    }
}
