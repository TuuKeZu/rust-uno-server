use crate::lobby::*;
use crate::messages::WsMessage;
use crate::packets::*;
use actix::prelude::{Actor, Context, Handler, Recipient};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
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
    pub players: Players,
    pub spectators: HashMap<Uuid, Player>,
    pub current_turn: Option<Uuid>,

    pub deck: VecDeque<Card>,
    pub placed_deck: VecDeque<Card>,
}

#[derive(Debug, Default)]
pub struct Players(VecDeque<(Uuid, Player)>);

impl Players {
    pub fn keys(&self) -> Vec<&Uuid> {
        self.0.iter().map(|pair| &pair.0).collect::<Vec<&Uuid>>()
    }

    pub fn keys_mut(&self) -> Vec<Uuid> {
        self.0.iter().map(|pair| pair.0).collect::<Vec<Uuid>>()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn get(&self, key: &Uuid) -> Option<&Player> {
        let x = self
            .0
            .iter()
            .find(|pair| pair.0 == *key)
            .map(|pair| &pair.1);
        x
    }

    pub fn get_mut(&mut self, key: &Uuid) -> Option<&mut Player> {
        let x = self
            .0
            .iter_mut()
            .find(|pair| pair.0 == *key)
            .map(|pair| &mut pair.1);
        x
    }

    pub fn contains_key(&self, key: &Uuid) -> bool {
        self.keys().contains(&key)
    }

    pub fn remove(&mut self, key: &Uuid) {
        // l
        self.0
            .remove(self.0.iter().position(|pair| pair.0 == *key).unwrap());
    }

    pub fn insert(&mut self, key: Uuid, player: Player) {
        self.0.push_back((key, player));
    }

    pub fn next(&mut self) -> Uuid {
        //k
        let current = self.0.pop_back().unwrap();
        self.0.push_front(current.clone());
        current.0
    }
}

impl Game {
    pub fn new() -> Game {
        Game {
            id: Uuid::new_v4(),
            active: false,
            players: Players::default(),
            spectators: HashMap::new(),
            deck: Card::generate_deck(),
            current_turn: None,
            placed_deck: VecDeque::new(),
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
            self.send_message(data, id);
        }
    }

    pub fn init_player(&mut self, id: &Uuid, username: &str) {
        let host = self.players.len() == 1;
        let p: Option<&mut Player> = self.players.get_mut(id);

        if let Some(p) = p {
            p.username = String::from(username);
            p.is_connected = true;
            p.is_host = host;
        }
    }

    pub fn start(&mut self) {
        let deck = &mut self.deck;
        self.placed_deck
            .push_front(Card::get_allowed_start_card(deck));

        self.active = true;

        for id in self.players.keys_mut() {
            self.draw_cards(8, id);
            self.update_card_status(&id);
        }

        self.give_turn();

        println!("Started!");
        println!("{:#?}", self.players);
    }

    pub fn give_turn(&mut self) {
        let current = self.next_turn();

        self.emit(
            &current,
            &MessagePacket::to_json(MessagePacket::new(
                &format!("{} is your own id", current)[..],
            )),
        );
        self.update_allowed_status(&current);
    }

    pub fn update_card_status(&self, self_id: &Uuid) {
        for id in self.players.keys() {
            if self_id == id {
                let p: PrivateGamePacket = PrivateGamePacket::new(
                    self.players.get(id).unwrap().cards.clone(),
                    self.placed_deck.iter().nth(0).unwrap().clone(),
                );
                self.emit(id, &PrivateGamePacket::to_json(p));
            } else {
                let p: PublicGamePacket = PublicGamePacket::new(
                    id.to_owned(),
                    self.players.get(id).unwrap().cards.len(),
                    self.placed_deck.iter().nth(0).unwrap().clone(),
                );
                self.emit(id, &PublicGamePacket::to_json(p));
            }
        }
    }

    pub fn update_allowed_status(&mut self, self_id: &Uuid) {
        let placed_deck = self.placed_deck.clone();
        let p = self.get_player(self_id);

        let allowed: Vec<Card> = Card::get_allowed_cards(
            placed_deck.iter().nth(0).unwrap().clone(),
            p.cards.clone(),
            self.current_turn.unwrap(),
        );

        let packet: AllowedCardsPacket = AllowedCardsPacket::new(allowed);
        self.send_message(&AllowedCardsPacket::to_json(packet), self_id);
    }

    pub fn draw_cards(&mut self, count: u8, owner: Uuid) {
        let mut l: Vec<Card> = Vec::new();
        let p = self.players.get_mut(&owner).unwrap();

        for _ in 0..count {
            if self.deck.len() == 0 {
                self.deck.extend(Card::generate_deck());
            }

            l.push(self.deck.pop_front().unwrap());
        }

        l.iter_mut().for_each(|card| card.owner = Some(owner));
        p.cards.extend(l);
    }

    pub fn next_turn(&mut self) -> Uuid {
        self.current_turn = Some(self.players.next());
        self.current_turn.unwrap()
    }

    pub fn place_card(&mut self, index: usize, id: Uuid) {
        let p = self.players.get_mut(&id).unwrap();

        self.placed_deck
            .push_front(p.cards.iter().nth(index).unwrap().clone());
        p.cards.remove(index);
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
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub r#type: Type,
    pub color: Color,
    pub owner: Option<Uuid>,
}

#[derive(strum_macros::Display, Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum Color {
    Red,
    Blue,
    Green,
    Yellow,
}

impl Color {
    pub fn iter() -> Vec<Color> {
        vec![Color::Red, Color::Blue, Color::Green, Color::Yellow]
    }
}

#[derive(strum_macros::Display, Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum Type {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Block,
    Reverse,
    DrawTwo,
    Switch,
    DrawFour,
}

impl Type {
    pub fn iter() -> Vec<Type> {
        vec![
            Type::One,
            Type::Two,
            Type::Three,
            Type::Four,
            Type::Five,
            Type::Six,
            Type::Seven,
            Type::Eight,
            Type::Nine,
            Type::Block,
            Type::Reverse,
            Type::DrawFour,
            Type::Switch,
            Type::DrawFour,
        ]
    }
}

impl Card {
    fn new(r#type: Type, color: Color) -> Card {
        Card {
            r#type,
            color,
            owner: None,
        }
    }

    fn generate_deck() -> VecDeque<Card> {
        let mut l: Vec<Card> = Vec::new();

        for c in &Color::iter() {
            for t in &Type::iter() {
                l.push(Card::new(t.clone(), c.clone()));
                l.push(Card::new(t.clone(), c.clone()));
            }
        }
        l.shuffle(&mut thread_rng());
        VecDeque::from(l)
    }

    fn get_allowed_start_card(deck: &VecDeque<Card>) -> Card {
        let disallowed_types = vec![
            Type::Block,
            Type::Switch,
            Type::DrawFour,
            Type::Reverse,
            Type::DrawTwo,
        ];

        deck.iter()
            .filter(|card| !disallowed_types.contains(&card.r#type))
            .collect::<VecDeque<&Card>>()
            .pop_back()
            .unwrap()
            .clone()
    }

    fn get_allowed_cards(last_card: Card, deck: Vec<Card>, owner: Uuid) -> Vec<Card> {
        let mut l = Vec::new();
        let special = [Type::Switch, Type::DrawFour];

        for card in deck {
            // SPECIAL CARDS
            if special.contains(&card.r#type) {
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
