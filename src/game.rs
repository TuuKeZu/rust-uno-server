use crate::lobby::*;
use crate::messages::WsMessage;
use crate::packets::*;
use actix::prelude::{Actor, Context, Handler, Recipient};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque, vec_deque};
use uuid::Uuid;


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
    pub current_turn: Option<Uuid>,

    pub deck: VecDeque<Card>,
}

impl Game {
    pub fn new() -> Game {
        Game {
            id: Uuid::new_v4(),
            active: false,
            players: HashMap::new(),
            spectators: HashMap::new(),
            deck: Card::generate_deck(),
            current_turn: None,
        }
    }

    pub fn leave(&mut self, id: Uuid) {
        if self.players.contains_key(&id) {
            self.players.remove(&id);
        }

        if self.spectators.contains_key(&id) {
            self.spectators.remove(&id);
        }
    }

    pub fn get_player(&mut self, id: &Uuid) -> &Player {
        self.players.get(id).unwrap()
    }

    pub fn get_spectator(&mut self, id: &Uuid) -> &Player {
        self.spectators.get(id).unwrap()
    }
}

impl Game {
    fn send_message(&self, message: &str, id: &Uuid) {
        if let Some(socket_recipient) = self.players.get(id) {
            let _ = socket_recipient
                .socket
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

    pub fn init_player(&mut self, id: &Uuid, username: &str) {
        let host = self.players.len() == 1;
        let p: Option<&mut Player> = self.players.get_mut(id);

        match p {
            Some(p) => {
                p.username = String::from(username);
                p.is_connected = true;
                p.is_host = host;
            }
            _ => {}
        }
    }

    pub fn start(&mut self) {
        let players: Vec<Uuid> = self.players.keys().cloned().collect::<Vec<Uuid>>();

        for id in players {
            self.draw_cards(8, id);
            self.update_card_status(&id);
        }

        self.give_turn();

        println!("Started!");
        println!("{:#?}", self.players);
    }

    pub fn give_turn(&mut self) {
        let current = self.next_turn();

        let mut deck = self.deck.clone();
        let p = self.get_player(&current);

        let allowed: Vec<Card> = Card::get_allowed_cards(
            deck.iter().nth(0).unwrap().clone(),
            p.cards.clone(),
            current,
        );

        let packet: AllowedCardsPacket = AllowedCardsPacket::new(allowed);
        self.send_message(&AllowedCardsPacket::to_json(packet), &current);
    }

    pub fn update_card_status(&self, self_id: &Uuid) {
        for id in self.players.keys() {
            if self_id == id {
                let p: PrivateGamePacket = PrivateGamePacket::new(
                    self.players.get(id).unwrap().cards.clone(),
                    self.deck.iter().nth(0).unwrap().clone(),
                );
                self.emit(id, &PrivateGamePacket::to_json(p));
            } else {
                let p: PublicGamePacket = PublicGamePacket::new(
                    id.to_owned(),
                    self.players.get(id).unwrap().cards.len(),
                    self.deck.iter().nth(0).unwrap().clone(),
                );
                self.emit(id, &PublicGamePacket::to_json(p));
            }
        }
    }

    pub fn draw_cards(&mut self, count: u8, owner: Uuid) {
        let mut l: Vec<Card> = Vec::new();
        let p = self.players.get_mut(&owner).unwrap();

        for _ in 0..count {

            if(self.deck.len() == 0) {
                self.deck.extend(Card::generate_deck());
            }

            l.push(self.deck.pop_front().unwrap());
        }

        l.iter_mut().for_each(|card| card.owner = Some(owner));
        p.cards.extend(l);
    }

    pub fn next_turn(&mut self) -> Uuid {
        let id: Option<&Uuid> = self.players.keys().next();

        match id {
            Some(_) => {
                self.current_turn = Some(Uuid::from(self.players.keys().next().unwrap().clone()));
                self.current_turn.unwrap()
            }
            _ => {
                self.current_turn = Some(Uuid::from(self.players.keys().nth(0).unwrap().clone()));
                self.current_turn.unwrap()
            }
        }
    }

    pub fn place_card(&mut self, card: Card) {
        self.deck.push_front(card);

    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub id: Uuid,
    pub socket: Socket,
    pub username: String,
    pub is_connected: bool,
    pub is_host: bool,
    pub cards: Vec<Card>,
}

impl Player {
    pub fn new(id: Uuid, socket: &Socket) -> Player {
        Player {
            id,
            socket: socket.to_owned(),
            username: String::from("connecting..."),
            is_host: false,
            is_connected: false,
            cards: Vec::new(),
        }
    }

    pub fn init(mut self, username: &str, is_host: bool) {
        self.username = String::from(username);
        self.is_host = is_host;
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub r#type: String,
    pub color: String,
    pub owner: Option<Uuid>,
}

impl Card {
    fn new(r#type: &str, color: &str) -> Card {
        Card {
            r#type: String::from(r#type),
            color: String::from(color),
            owner: None,
        }
    }

    fn generate_deck() -> VecDeque<Card> {
        let mut l: Vec<Card> = Vec::new();
        let types: [&str; 15] = [
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "BLOCK", "REVERSE", "DRAW-2",
            "SWICTH", "DRAW-4",
        ];
        let colors: [&str; 4] = ["RED", "YELLOW", "BLUE", "GREEN"];

        for c in colors {
            for t in types {
                l.push(Card::new(t, c));
                l.push(Card::new(t, c));
            }
        }
        l.shuffle(&mut thread_rng());
        VecDeque::from(l)
    }

    fn get_allowed_cards(last_card: Card, deck: Vec<Card>, owner: Uuid) -> Vec<Card> {
        let mut l: Vec<Card> = Vec::new();
        let allowed_types: [String; 2] = ["SWICTH".to_string(), "DRAW-4".to_string()];

        for card in deck {
            // SPECIAL CARDS
            if allowed_types.contains(&card.r#type) {
                l.push(card);
                continue;
            }

            // SAME COLORED CARDSS
            if card.color == last_card.color && card.owner != Some(owner) {
                l.push(card);
                continue;
            }

            // SAME NUMBERS
            if card.r#type == last_card.r#type {
                l.push(card);
                continue;
            }
        }

        l
    }
}
