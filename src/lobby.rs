use crate::errors::HTMLError;
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
                    lobby
                        .game
                        .broadcast(&MessagePacket::to_json(MessagePacket::new(
                            &format!("{} Disconnected", player.username)[..],
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
                // TODO allow spectators. Currently they are sent an HTMLError

                let _ = &packet
                    .addr
                    .do_send(WsMessage(HTMLError::to_json(HTMLError::new(
                        401,
                        "Cannot join active game.",
                    ))));
                return;
            }

            println!("Connection is waiting to join...");

            room.game
                .players
                .insert(packet.self_id, Player::new(packet.self_id, &packet.addr));

            room.game.emit(
                &packet.self_id,
                &MessagePacket::to_json(MessagePacket::new(
                    &format!("{} is your own id", &packet.self_id)[..],
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

            // Confirm that the packet has a type
            if packet.json.get("type").is_some() {
                // Resolve the packet's type
                let r#type: String = packet.json.get("type").unwrap().to_string();

                // Huge match-tree
                match &r#type as &str {
                    "\"ERROR\"" => {
                        room.game
                            .emit(&packet.id, &serde_json::to_string(&packet.json).unwrap());
                    }

                    // Register-event
                    "\"REGISTER\"" => {
                        let p: Result<RegisterPacket> = RegisterPacket::try_parse(&packet.data);

                        match p {
                            Ok(data) => {
                                if room.game.get_player(&packet.id).is_connected {
                                    room.game.emit(
                                        &packet.id,
                                        &HTMLError::to_json(HTMLError::new(
                                            401,
                                            "Instance already exists.",
                                        )),
                                    );
                                    return;
                                }

                                room.game.init_player(&packet.id, &data.username);
                                room.game
                                    .broadcast(&MessagePacket::to_json(MessagePacket::new(
                                        &format!("{} Joined the game.", data.username)[..],
                                    )));
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(401, &e.to_string())),
                                );
                            }
                        }
                    }

                    // Message-event
                    "\"MESSAGE\"" => {
                        let p: Result<MessagePacket> = MessagePacket::try_parse(&packet.data);

                        match p {
                            Ok(data) => {
                                room.game
                                    .broadcast(&MessagePacket::to_json(MessagePacket::new(
                                        data.content.as_str(),
                                    )));
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                                );
                            }
                        }
                    }

                    // Start-event
                    "\"START-GAME\"" => {
                        let p: Result<StartPacket> = StartPacket::try_parse(&packet.data);

                        let host: bool = room.game.get_player(&packet.id).is_host;

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

                        if room.game.players.len() < 2 {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(
                                    401,
                                    "The game required at least 2 players to start.",
                                )),
                            );
                            return;
                        }

                        match p {
                            Ok(_) => {
                                room.game
                                    .broadcast(&MessagePacket::to_json(MessagePacket::new(
                                        "Starting game, good luck!",
                                    )));
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

                    // Draw-event
                    "\"DRAW-CARDS\"" => {
                        let p: Result<DrawPacket> = DrawPacket::try_parse(&packet.data);

                        // Disallow request unless the player has the turn
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(401, "It's not yout turn.")),
                            );
                        }

                        match p {
                            Ok(data) => {
                                room.game.draw_cards(data.amount, packet.id);
                                room.game.update_card_status(&packet.id);
                                room.game.update_allowed_status(&packet.id);
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                                );
                            }
                        }
                    }

                    // Place-event
                    "\"PLACE-CARD\"" => {
                        let p: Result<PlaceCardPacket> = PlaceCardPacket::try_parse(&packet.data);

                        // Disallow request unless the player has the turn
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(401, "It's not yout turn.")),
                            );
                        }

                        match p {
                            Ok(data) => {
                                if data.index
                                    > room.game.players.get(&packet.id).unwrap().cards.len() - 1
                                {
                                    room.game.emit(
                                        &packet.id,
                                        &HTMLError::to_json(HTMLError::new(
                                            400,
                                            "Card at index was not found.",
                                        )),
                                    );
                                    return;
                                }

                                room.game.place_card(data.index, packet.id);
                                room.game.update_card_status(&packet.id);
                                room.game.update_allowed_status(&packet.id);
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                                );
                            }
                        }
                    }

                    // Called in the end of each turn
                    "\"END-TURN\"" => {
                        let p: Result<EndTurnPacket> = EndTurnPacket::try_parse(&packet.data);

                        // Disallow request unless the player has the turn
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(401, "It's not yout turn.")),
                            );
                        }

                        match p {
                            Ok(_) => {
                                room.game.end_turn();
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                                );
                            }
                        }
                    }
                    "\"COLOR-SWITCH\"" => {
                        let p: Result<ColorSwitchPacket> =
                            ColorSwitchPacket::try_parse(&packet.data);

                        // Disallow request unless the player has the turn
                        if room.game.current_turn.unwrap_or_default() != packet.id {
                            room.game.emit(
                                &packet.id,
                                &HTMLError::to_json(HTMLError::new(401, "It's not yout turn.")),
                            );
                        }

                        match p {
                            Ok(data) => {
                                room.game.switch_color(data.color);
                                room.game.update_card_status(&packet.id);
                                room.game.update_allowed_status(&packet.id);
                            }
                            Err(e) => {
                                room.game.emit(
                                    &packet.id,
                                    &HTMLError::to_json(HTMLError::new(400, &e.to_string())),
                                );
                            }
                        }
                    }

                    &_ => {}
                }
            } else {
                room.game.emit(
                    &packet.id,
                    &HTMLError::to_json(HTMLError::new(400, "Missing request type.")),
                );
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
