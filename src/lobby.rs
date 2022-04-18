use crate::messages::{Packet, Connect, Disconnect, WsMessage};
use crate::errors::HTMLError;
use crate::packets::*;
use actix::prelude::{Actor, Context, Handler, Recipient};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use serde_json::Result;


type Socket = Recipient<WsMessage>;

pub struct Lobby {
    sessions: HashMap<Uuid, Socket>,
    rooms: HashMap<Uuid, HashSet<Uuid>>
}

impl Default for Lobby {
    fn default() -> Lobby {
        Lobby {
            sessions: HashMap::new(),
            rooms: HashMap::new()
        }
    }
}

impl Lobby {
    fn send_message(&self, message: &str, id_to: &Uuid) {
        if let Some(socket_recipient) = self.sessions.get(id_to) {
            let _ = socket_recipient
                .do_send(WsMessage(message.to_owned()));
        } else {
            println!("Couldn't find anyone to send message to");
        }
    }

    pub fn emit(&mut self, packet: &Packet, data: &str) {
        self.send_message(data, &packet.id);
    }

    pub fn broadcast(&mut self, packet: &Packet, data: &str) {
        self.rooms.get(&packet.room_id).unwrap().iter().for_each(|client| self.send_message(data, client))
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
            .iter()
            .filter(|conn_id| *conn_id.to_owned() != packet.id)
            .for_each(|user_id| self
                .send_message(&format!("{} disconnected.", packet.id), user_id
            ));

            println!("[{}] User Disconnected: {}",  packet.room_id, packet.id);

            if let Some(lobby) = self.rooms.get_mut(&packet.room_id) {
                if lobby.len() > 1 {
                    lobby.remove(&packet.id);
                }
                else{
                    self.rooms.remove(&packet.room_id);
                }
            }
        }
    }
}

impl Handler<Connect> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Connect, _: &mut Context<Self>) -> Self::Result {
        self.rooms.entry(packet.lobby_id).or_insert_with(HashSet::new).insert(packet.self_id);

        self
            .rooms
            .get(&packet.lobby_id)
            .unwrap()
            .iter()
            .filter(|conn_id| *conn_id.to_owned() != packet.self_id)
            .for_each(|conn_id| self.send_message(&format!("{} just joined!", packet.self_id), conn_id));

        self.sessions.insert(
            packet.self_id,
            packet.addr,
        );

        self.send_message(&format!("your id is {}", packet.self_id), &packet.self_id);
        println!("[{}] New user Joined: {}", packet.lobby_id, packet.self_id);
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

                "\"MESSAGE\"" => {
                    let p: Result<MessagePacket> = MessagePacket::try_parse(&packet.data);

                    match p {
                        Ok(data) => {
                            self.broadcast(&packet, &data.content);
                        }
                        Err(e) => {
                            self.emit(&packet, &HTMLError::to_json(HTMLError::new(401, &e.to_string())) );
                        }
                    }
                    
                }

                &_ => {
                    println!("Unknown type.");
                }
            }
        }
        else { 
            self.send_message(&HTMLError::to_json(HTMLError::new(400, "Missing request type.")), &packet.id);
        }

        println!("DEBUG: [{}] {} > {:?} ", packet.room_id, packet.id, packet.json)
    }
}