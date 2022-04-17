use crate::messages::{Packet, Connect, Disconnect, WsMessage};
use actix::prelude::{Actor, Context, Handler, Recipient};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

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
        println!("[{}] New user Joined: {}", packet.lobby_id, packet.self_id)
    }
}

impl Handler<Packet> for Lobby {
    type Result = ();

    fn handle(&mut self, packet: Packet, _ctx: &mut Context<Self>) -> Self::Result {
        if packet.data.starts_with("\\w") {
            if let Some(id_to) = packet.data.split(' ').collect::<Vec<&str>>().get(1) {
                self.send_message(&packet.data, &Uuid::parse_str(id_to).unwrap());
            }
        } else {
            self.rooms.get(&packet.room_id).unwrap().iter().for_each(|client| self.send_message(&packet.data, client));
            println!("[{}] {} > {:?} ", packet.room_id, packet.id, packet.json)
        }
    }
}