use crate::lobby::*;
use uuid::Uuid;
use std::collections::{HashMap};
use rand::thread_rng;
use rand::seq::SliceRandom;
use actix::prelude::{Actor, Context, Handler, Recipient};
use crate::messages::WsMessage;

// https://www.unorules.org/wp-content/uploads/2021/03/All-Uno-cards-how-many-cards-in-uno.png
/*
CARD-TYPES
0 - 0
1 - 1
2 - 2
3 - 3
4 - 4
5 - 5
6 - 6
7 - 7
8 - 8
9 - 9
10 - block
11 - reverse
12 - +2
13 - switch color
14 - +4
COLOR-TYPES
0 - red
1 - yellow
2 - blue
3 - green
*/

type Socket = Recipient<WsMessage>;

#[derive(Debug)]
pub struct Game {
    pub id: Uuid,
    pub active: bool,
    pub players: HashMap<Uuid, Player>,
    pub spectators: HashMap<Uuid, Player>,

    pub deck: Vec<Card>,
}

impl Game {
    pub fn new() -> Game {
        Game {
            id: Uuid::new_v4(),
            active: false,
            players: HashMap::new(),
            spectators: HashMap::new(),
            deck: Card::generate_deck()
        }
    }

    pub fn join(&mut self, id: Uuid, player: Player) {
        self.players.insert(id, player);
    }

    pub fn spectate(&mut self, id: Uuid, player: Player) {
        self.players.insert(id, player);
    }

    pub fn leave(&mut self, id: Uuid) {
        if self.players.contains_key(&id) {
            self.players.remove(&id);
        }

        if self.spectators.contains_key(&id) {
            self.spectators.remove(&id);
        }
    }
}


impl Game {

    fn send_message(&self, message: &str, id: &Uuid) {

        if let Some(socket_recipient) = self.players.get(id){
            let _ = socket_recipient.socket
                .do_send(WsMessage(message.to_owned()));
            
        } else {
            println!("Couldn't find anyone to send message to");
        }

    }

    pub fn emit(&self, id: &Uuid, data: &str) {
        self.send_message(data, id);
    }

    pub fn broadcast(&self, data: &str) {
        for id in self.players.keys() {
            self.send_message(data, &id);
        }
    }

    pub fn start(&mut self) {
        let players: Vec<Uuid> = self.players.keys().cloned().collect::<Vec<Uuid>>();

        for id in players {
            self.players.get_mut(&id).unwrap().cards = self.draw_cards(8);

            self.update_card_status(&id);
        }

        println!("Started!");
        println!("{:#?}", self.players);
    }

    pub fn update_card_status(&self, self_id: &Uuid) {
        for id in self.players.keys() {

            if self_id == id {
                self.emit(id, "here's my cards");
            }
            else {
                self.emit(id, "here's someone elses cards");
            }

        } 
    }

    pub fn draw_cards(&mut self, count: u8) -> Vec<Card> {
        let mut l: Vec<Card> = Vec::new();

        for i in 0..count {
            l.push(self.deck.pop().unwrap());
        }

        l
    }
}


#[derive(Debug)]
#[derive(Clone)]
pub struct Player {
    pub id: Uuid,
    pub socket: Socket,
    pub username: String,
    pub is_host: bool,
    pub cards: Vec<Card>,
}

impl Player {
    pub fn new(id: Uuid, socket: &Socket, is_host: bool, username: &str) -> Player {
        Player {
            id,
            socket: socket.to_owned(),
            username: String::from(username),
            is_host,
            cards: Vec::new()
        }
    }
}
#[derive(Debug)]
#[derive(Clone)]
pub struct Card {
    pub r#type: String,
    pub color: String,
}

impl Card { 
    fn new(r#type: &str, color: &str) -> Card {
        Card {
            r#type: String::from(r#type),
            color: String::from(color)
        }
    }

    fn generate_deck() -> Vec<Card> {
        let mut l: Vec<Card> = Vec::new();
        let types: [&str; 15] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "BLOCK", "REVERSE", "DRAW-2", "SWICTH", "DARW-4"];
        let colors: [&str; 4] = ["RED", "YELLOW", "BLUE", "GREEN"];
        
        for c in colors {
            for t in types {
                l.push( Card::new(t, c) );
                l.push( Card::new(t, c) );
            }
        }
        l.shuffle(&mut thread_rng());
        l
    }
}