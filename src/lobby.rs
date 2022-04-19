use crate::errors::HTMLError;
use crate::game::{Card, Game, Player};
use crate::messages::{Connect, Disconnect, Packet, WsMessage};
use crate::packets::*;
use actix::prelude::{Actor, Context, Handler, Recipient};
use serde_json::Result;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

type Socket = Recipient<WsMessage>;

#[derive(Debug)]
pub struct Lobby {
    sessions: HashMap<Uuid, Socket>,
    rooms: HashMap<Uuid, Room>,
}

#[derive(Debug)]
pub struct Room {
    connections: HashSet<Uuid>,
    game: Game,
}

impl Room {
    fn new() -> Room {
        Room {
            connections: HashSet::new(),
            game: Game::new(),
        }
    }
}

impl Default for Lobby {
    fn default() -> Lobby {
        Lobby {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
        }
    }
}

impl Lobby {
    fn send_message(&self, message: &str, id_to: &Uuid) {
        if let Some(socket_recipient) = self.sessions.get(id_to) {
            let t = socket_recipient.do_send(WsMessage(message.to_owned()));
        } else {
            println!("Couldn't find anyone to send message to");
        }
    }

    pub fn emit(&mut self, packet: &Packet, data: &str) {
        self.send_message(data, &packet.id);
    }

    pub fn broadcast(&mut self, packet: &Packet, data: &str) {
        self.rooms
            .get(&packet.room_id)
            .unwrap()
            .connections
            .iter()
            .for_each(|client| self.send_message(data, client))
    }

    pub fn player_exists(&mut self, room_id: &Uuid, id: &Uuid) -> bool {
        self.rooms
            .get(room_id)
            .unwrap()
            .game
            .players
            .contains_key(id)
    }

    pub fn get_player(&mut self, room_id: &Uuid, id: &Uuid) -> &Player {
        self.rooms
            .get(room_id)
            .unwrap()
            .game
            .players
            .get(id)
            .unwrap()
    }

    pub fn get_spectator(&mut self, room_id: &Uuid, id: &Uuid) -> &Player {
        self.rooms
            .get(room_id)
            .unwrap()
            .game
            .spectators
            .get(id)
            .unwrap()
    }
}

impl Actor for Lobby {
    type Context = Context<Self>;
}

impl Handler<Disconnect> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Disconnect, _: &mut Context<Self>) {
        if self.sessions.remove(&packet.id).is_some() {
            self.rooms
                .get(&packet.room_id)
                .unwrap()
                .connections
                .iter()
                .filter(|conn_id| *conn_id.to_owned() != packet.id)
                .for_each(|user_id| {
                    self.send_message(&format!("{} disconnected.", packet.id), user_id)
                });

            println!("[{}] Disconnected: {}", packet.room_id, packet.id);

            if let Some(lobby) = self.rooms.get_mut(&packet.room_id) {
                if lobby.connections.len() > 1 {
                    lobby.connections.remove(&packet.id);
                    lobby.game.leave(packet.id);
                } else {
                    self.rooms.remove(&packet.room_id);
                }
            }
        }
    }
}

impl Handler<Connect> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Connect, _: &mut Context<Self>) -> Self::Result {
        self.rooms
            .entry(packet.lobby_id)
            .or_insert_with(Room::new)
            .connections
            .insert(packet.self_id);

        self.rooms
            .get(&packet.lobby_id)
            .unwrap()
            .connections
            .iter()
            .filter(|conn_id| *conn_id.to_owned() != packet.self_id)
            .for_each(|conn_id| {
                self.send_message(&format!("{} just joined!", packet.self_id), conn_id)
            });

        self.sessions.insert(packet.self_id, packet.addr);

        self.send_message(&format!("your id is {}", packet.self_id), &packet.self_id);
        println!(
            "[{}] User is connecting... ({})",
            packet.lobby_id, packet.self_id
        );
    }
}

impl Handler<Packet> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Packet, _ctx: &mut Context<Self>) -> Self::Result {
        if packet.json.get("type").is_some() {
            let r#type: String = packet.json.get("type").unwrap().to_string();

            match &r#type as &str {
                "\"ERROR\"" => {
                    self.emit(&packet, &serde_json::to_string(&packet.json).unwrap());
                }

                "\"REGISTER\"" => {
                    let p: Result<RegisterPacket> = RegisterPacket::try_parse(&packet.data);

                    match p {
                        Ok(data) => {
                            let host: bool =
                                self.rooms.get(&packet.room_id).unwrap().game.players.len() == 0;
                            let started: bool =
                                self.rooms.get(&packet.room_id).unwrap().game.active;

                            if self
                                .rooms
                                .get(&packet.room_id)
                                .unwrap()
                                .game
                                .players
                                .contains_key(&packet.id)
                            {
                                self.emit(
                                    &packet,
                                    &HTMLError::to_json(HTMLError::new(
                                        400,
                                        "Instance already exists for this websocket.",
                                    )),
                                );
                                return;
                            }

                            if !started {
                                let p: Player = Player::new(
                                    packet.id,
                                    self.sessions.get(&packet.id).unwrap(),
                                    host,
                                    &data.username,
                                );

                                self.rooms
                                    .get_mut(&packet.room_id)
                                    .unwrap()
                                    .game
                                    .join(packet.id, p);
                                self.broadcast(
                                    &packet,
                                    &format!("{} has joined the game", &data.username),
                                );
                                return;
                            } else {
                                self.rooms.get_mut(&packet.room_id).unwrap().game.spectate(
                                    packet.id,
                                    Player::new(
                                        packet.id,
                                        self.sessions.get(&packet.id).unwrap(),
                                        host,
                                        &data.username,
                                    ),
                                );
                                self.broadcast(
                                    &packet,
                                    &format!("{} has joined as spectator.", &data.username),
                                );
                                return;
                            }
                        }
                        Err(e) => {
                            self.emit(
                                &packet,
                                &HTMLError::to_json(HTMLError::new(401, &e.to_string())),
                            );
                        }
                    }
                }

                "\"MESSAGE\"" => {
                    let p: Result<MessagePacket> = MessagePacket::try_parse(&packet.data);

                    if !self.player_exists(&packet.room_id, &packet.id) {
                        self.emit(
                            &packet,
                            &HTMLError::to_json(HTMLError::new(
                                401,
                                "Only registered players can perform actions.",
                            )),
                        );
                        return;
                    }

                    match p {
                        Ok(data) => {
                            self.broadcast(&packet, &data.content);
                        }
                        Err(e) => {
                            self.emit(
                                &packet,
                                &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                            );
                        }
                    }
                }

                "\"START-GAME\"" => {
                    let p: Result<StartPacket> = StartPacket::try_parse(&packet.data);

                    if !self.player_exists(&packet.room_id, &packet.id) {
                        self.emit(
                            &packet,
                            &HTMLError::to_json(HTMLError::new(
                                401,
                                "Only registered players can perform actions.",
                            )),
                        );
                        return;
                    }

                    let host: bool = self.get_player(&packet.room_id, &packet.id).is_host;

                    if !host {
                        self.emit(
                            &packet,
                            &HTMLError::to_json(HTMLError::new(
                                401,
                                "Only host can start the game.",
                            )),
                        );
                        return;
                    }

                    match p {
                        Ok(_) => {
                            self.broadcast(&packet, "Starting the game, Good luck!");
                            self.rooms.get_mut(&packet.room_id).unwrap().game.start();
                        }
                        Err(e) => {
                            self.emit(
                                &packet,
                                &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                            );
                        }
                    }
                }

                &_ => {
                    println!("Unknown type.");
                }
            }
        } else {
            self.send_message(
                &HTMLError::to_json(HTMLError::new(400, "Missing request type.")),
                &packet.id,
            );
        }

        println!(
            "DEBUG: [{}] {} > {:?} ",
            packet.room_id, packet.id, packet.json
        )
    }
}
