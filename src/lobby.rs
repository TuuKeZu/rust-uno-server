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
    fn send_message(&self, message: &str, room_id: &Uuid, id_to: &Uuid) {
        if let Some(player) = self.rooms.get(room_id).unwrap().game.players.get(id_to) {
            let t = player.socket.do_send(WsMessage(message.to_owned()));
        } else {
            println!("Couldn't find anyone to send message to");
        }
    }
    /*
    pub fn emit(&self, packet: &Packet, data: &str) {
        self.send_message(data, &packet.room_id, &packet.id);
    }

    pub fn broadcast(&self, packet: &Packet, data: &str) {
        self.rooms
            .get(&packet.room_id)
            .unwrap()
            .connections
            .iter()
            .for_each(|client| self.send_message(data, &packet.room_id, client))
    }
    */

    pub fn player_exists(&self, room_id: &Uuid, id: &Uuid) -> bool {
        let p: Option<&Player> = self.rooms
            .get(room_id)
            .unwrap()
            .game
            .players
            .get(id);

        match p {
            Some(p) => p.is_connected,
            _ => false
        }

    }
}

impl Actor for Lobby {
    type Context = Context<Self>;
}

impl Handler<Disconnect> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Disconnect, _: &mut Context<Self>) {
        
        if let Some(lobby) = self.rooms.get_mut(&packet.room_id) {
            if lobby.game.players.len() > 1 {
                lobby.game.leave(packet.id);

                lobby.game.broadcast(&format!("{} disconnected.", packet.id));

            } else {
                self.rooms.remove(&packet.room_id);
            }
        }

        /*
        self.rooms
            .get(&packet.room_id)
            .unwrap()
            .connections
            .iter()
            .filter(|conn_id| *conn_id.to_owned() != packet.id)
            .for_each(|user_id| {
                self.send_message(&format!("{} disconnected.", packet.id), user_id)
            });
        */
    }
}

impl Handler<Connect> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Connect, _: &mut Context<Self>) -> Self::Result {
        if !self.rooms.contains_key(&packet.lobby_id) {
            self.rooms.insert(packet.lobby_id, Room::new());
        }

        if let Some(room) = self.rooms.get_mut(&packet.lobby_id) {

            room.game.broadcast(&format!("{} is waiting to join the game...", packet.self_id));
    
            self.sessions.insert(packet.self_id, packet.addr);
            room.game.players.insert(packet.self_id, Player::new(packet.self_id, self.sessions.get(&packet.self_id).unwrap()));
    
            room.game.emit(&packet.self_id, &format!("{} is your own id", packet.self_id));
        }
    }
}

impl Handler<Packet> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Packet, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(room) = self.rooms.get_mut(&packet.room_id) {

            if packet.json.get("type").is_some() {
                let r#type: String = packet.json.get("type").unwrap().to_string();
    
                match &r#type as &str {
                    "\"ERROR\"" => {
                        room.game.emit(&packet.id, &serde_json::to_string(&packet.json).unwrap());
                    }
    
                    "\"REGISTER\"" => {
                        let p: Result<RegisterPacket> = RegisterPacket::try_parse(&packet.data);
    
                        match p {
                            Ok(data) => {
                                /*
                                if self.player_exists(&packet.room_id, &packet.id) {
                                    room.game.emit(
                                        &packet.id,
                                        &HTMLError::to_json(HTMLError::new(
                                            401,
                                            "Instance already exists.",
                                        )),
                                    );
                                    return;
                                }
                                */
                                room.game.init_player(&packet.id, "test");
                                room.game.broadcast(&format!("{} has joined.", &data.username));
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(401, &e.to_string())),
                                );
                            }
                        }
                    }
    
                    "\"MESSAGE\"" => {
                        let p: Result<MessagePacket> = MessagePacket::try_parse(&packet.data);
                        /*
                        if !self.player_exists(&packet.room_id, &packet.id) {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(
                                    401,
                                    "Only registered players can perform actions.",
                                )),
                            );
                            return;
                        }
                        */
                        match p {
                            Ok(data) => {
                                room.game.broadcast(&data.content);
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                                );
                            }
                        }
                    }
    
                    "\"START-GAME\"" => {
                        let p: Result<StartPacket> = StartPacket::try_parse(&packet.data);
                        /*
                        if !self.player_exists(&packet.room_id, &packet.id) {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(
                                    401,
                                    "Only registered players can perform actions.",
                                )),
                            );
                            return;
                        }
                        */
                        let host: bool = room.game.get_player(&packet.room_id, &packet.id).is_host;
    
                        if !host {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(
                                    401,
                                    "Only host can start the game.",
                                )),
                            );
                            return;
                        }
    
                        match p {
                            Ok(_) => {
                                room.game.broadcast("Starting the game, Good luck!");
                                self.rooms.get_mut(&packet.room_id).unwrap().game.start();
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
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
                room.game.emit(&packet.id, &HTMLError::to_json(HTMLError::new(400, "Missing request type.")));
            }
        }
        else {
            println!("{:?}", self.rooms);
        }

        println!(
            "DEBUG: [{}] {} > {:?} ",
            packet.room_id, packet.id, packet.json
        )
    }
}
