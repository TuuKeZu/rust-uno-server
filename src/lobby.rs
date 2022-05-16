use crate::game::{Game, Player};
use crate::messages::{Connect, Disconnect, Packet, WsMessage};
use crate::packets::*;
use actix::prelude::{Actor, Context, Handler};
use serde_json::Result;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct Lobby {
    rooms: HashMap<Uuid, Room>,
}

#[derive(Debug)]
pub struct Room {
    game: Game,
}

impl Room {
    fn new() -> Room {
        Room { game: Game::new() }
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
                let disconnected = lobby.game.players.get(&packet.id);

                if let Some(player) = disconnected {
                    lobby.game.broadcast(&to_json(PacketType::Disconnect(
                        packet.id,
                        player.username.clone(),
                    )));

                    lobby.game.leave(packet.id);
                }
            } else {
                self.rooms.remove(&packet.room_id);
            }
        }
    }
}

impl Handler<Connect> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Connect, _: &mut Context<Self>) -> Self::Result {
        // A very sexy one-liner
        self.rooms.entry(packet.lobby_id).or_insert_with(Room::new);

        if let Some(room) = self.rooms.get_mut(&packet.lobby_id) {
            if room.game.active {
                // TODO allow spectators. Currently they are sent an HTMLError when trying to join

                let _ = &packet.addr.do_send(WsMessage(to_json(PacketType::Error(
                    401,
                    "Game you are trying to join has already started".to_string(),
                ))));
                return;
            }

            println!("Connection is waiting to join...");

            room.game
                .players
                .insert(packet.self_id, Player::new(packet.self_id, &packet.addr));

            room.game.emit(
                &packet.self_id,
                &to_json(PacketType::Message(
                    "Server".to_string(),
                    format!("{} is your own id", &packet.self_id),
                )),
            );
        }
    }
}

impl Handler<Packet> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Packet, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(room) = self.rooms.get_mut(&packet.room_id) {
            // Ignore all the request sent by non-players
            if !room.game.players.contains_key(&packet.id) {
                return;
            }
            let data: Result<PacketType> = serde_json::from_str(&packet.data);

            if let Ok(packet_data) = data {
                match packet_data {
                    PacketType::Register(username) => {
                        if room.game.get_player(&packet.id).is_connected {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(
                                    401,
                                    "Instance already exists".to_string(),
                                )),
                            );
                            return;
                        }
                        // Initialize the player
                        room.game.init_player(&packet.id, &username);

                        // Broadcast the join-event
                        room.game.broadcast_ignore_self(
                            packet.id,
                            &to_json(PacketType::Connect(packet.id, username.clone())),
                        );

                        // Emit the current game-data to the player
                        room.game.emit(
                            &packet.id,
                            &to_json(PacketType::GameData(
                                packet.id,
                                username,
                                room.game.players.map_username(),
                            )),
                        )
                    }
                    PacketType::GameData(_, _, _) => {} // Will only be sent to client
                    PacketType::Connect(_, _) => {}     // Will only be sent to client
                    PacketType::Disconnect(_, _) => {}  // Will only be sent to client
                    PacketType::Message(sender, content) => {
                        room.game
                            .broadcast(&to_json(PacketType::Message(sender, content)));
                    }
                    PacketType::StartGame(_options) => {
                        let host: bool = room.game.get_player(&packet.id).is_host;

                        if !host {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(
                                    401,
                                    "You cannot start the game".to_string(),
                                )),
                            );
                            return;
                        }

                        if room.game.players.len() < 2 {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(
                                    401,
                                    "Cannot start the game alone".to_string(),
                                )),
                            );
                            return;
                        }

                        self.rooms.get_mut(&packet.room_id).unwrap().game.start();
                    }
                    PacketType::StatusUpdatePublic(_, _, _, _) => {} // Will only be sent to client
                    PacketType::StatusUpdatePrivate(_, _) => {}      // Will only be sent to client
                    PacketType::AllowedCardsUpdate(_) => {}          // Will only be sent to client
                    PacketType::DrawCard(amount) => {
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(401, "It's not your turn".to_string())),
                            );
                            return;
                        }

                        room.game.draw_cards(amount.into(), packet.id);
                        room.game.update_card_status(&packet.id);
                        room.game.update_allowed_status(&packet.id);
                    }
                    PacketType::PlaceCard(index) => {
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(401, "It's not your turn".to_string())),
                            );
                            return;
                        }

                        room.game.place_card(index, packet.id);
                        room.game.update_card_status(&packet.id);
                        room.game.update_allowed_status(&packet.id);
                    }
                    PacketType::EndTurn => {
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(401, "It's not your turn".to_string())),
                            );
                            return;
                        }

                        room.game.end_turn(packet.id);
                    }
                    PacketType::ColorSwitch(color) => {
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &to_json(PacketType::Error(401, "It's not your turn".to_string())),
                            );
                            return;
                        }

                        room.game.switch_color(color);
                        room.game.update_card_status(&packet.id);
                        room.game.update_allowed_status(&packet.id);
                    }
                    PacketType::TurnUpdate(_, _) => {} // Will only be sent to client
                    PacketType::Error(_, _) => {}
                    PacketType::WinUpdate(_, _, _, _) => {} // Will only be sent to client
                }
            }
        } else {
            println!("{:?}", self.rooms);
        }

        println!(
            "DEBUG: [{}] {} > {:?} ",
            packet.room_id, packet.id, packet.json
        )
    }
}

pub fn to_json(data: PacketType) -> String {
    serde_json::to_string(&data).unwrap()
}
