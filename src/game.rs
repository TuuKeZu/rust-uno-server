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

    pub draw_stack: usize,
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

    pub fn next_player(&mut self) -> Uuid {
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
            draw_stack: 0,
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

        if host {
            self.emit(
                id,
                &MessagePacket::to_json(MessagePacket::new("You are the host")),
            )
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
    }

    pub fn give_turn(&mut self) {
        let current = self.next_turn();

        self.emit(
            &current,
            &MessagePacket::to_json(MessagePacket::new("Your turn.")),
        );

        self.update_allowed_status(&current);
    }

    pub fn end_turn(&mut self) {
        let draw_cards = [Type::DrawTwo, Type::DrawFour];
        let last_card = self.placed_deck.get(0).unwrap();

        if last_card.owner != self.current_turn && draw_cards.contains(&last_card.r#type) {
            let mut count = if last_card.r#type == Type::DrawFour {
                4
            } else {
                2
            };

            if self.draw_stack >= count {
                count = self.draw_stack;
            }

            self.draw_cards(count, self.current_turn.unwrap());
            self.placed_deck.get_mut(0).unwrap().owner = None;
        } else {
            self.placed_deck.get_mut(0).unwrap().owner = self.current_turn;
        }

        self.update_card_status(&self.current_turn.unwrap());
        self.give_turn();
    }

    pub fn update_card_status(&self, self_id: &Uuid) {
        for id in self.players.keys() {
            if self_id == id {
                let p: PrivateGamePacket = PrivateGamePacket::new(
                    self.players.get(id).unwrap().cards.clone(),
                    self.placed_deck.get(0).unwrap().clone(),
                );
                self.emit(id, &PrivateGamePacket::to_json(p));
            } else {
                let p: PublicGamePacket = PublicGamePacket::new(
                    self_id.to_owned(),
                    &self.players.get(self_id).unwrap().username,
                    self.players.get(self_id).unwrap().cards.len(),
                    self.placed_deck.get(0).unwrap().clone(),
                );
                self.emit(id, &PublicGamePacket::to_json(p));
            }
        }
    }

    pub fn update_allowed_status(&mut self, self_id: &Uuid) {
        println!("Updating allowed status");

        let placed_deck = self.placed_deck.clone();

        let p = self.get_player(self_id);

        let allowed: Vec<Card> = Card::get_allowed_cards(
            placed_deck.get(0).unwrap().clone(),
            p.cards.clone(),
            self.current_turn.unwrap(),
        );

        let packet: AllowedCardsPacket = AllowedCardsPacket::new(allowed);
        self.send_message(&AllowedCardsPacket::to_json(packet), self_id);
    }

    pub fn draw_cards(&mut self, count: usize, owner: Uuid) {
        let mut l: Vec<Card> = Vec::new();
        let p = self.players.get_mut(&owner).unwrap();

        for _ in 0..count {
            if self.deck.is_empty() {
                self.deck.extend(Card::generate_deck());
            }

            l.push(self.deck.pop_front().unwrap());
        }
        l.push(Card::new(Type::DrawFour, Color::Red));

        l.iter_mut().for_each(|card| card.owner = Some(owner));
        p.cards.extend(l);
    }

    pub fn next_turn(&mut self) -> Uuid {
        self.current_turn = Some(self.players.next_player());
        self.current_turn.unwrap()
    }

    pub fn place_card(&mut self, index: usize, id: Uuid) {
        let draw_cards = [Type::DrawTwo, Type::DrawFour];
        let p = self.players.get_mut(&id).unwrap();

        // Stacked draw-cards
        if self.placed_deck.get(0).unwrap().r#type == p.cards.get(index).unwrap().r#type
            && draw_cards.contains(&self.placed_deck.get(0).unwrap().r#type)
        {
            let count = if self.placed_deck.get(0).unwrap().r#type == Type::DrawFour {
                4
            } else {
                2
            };
            if self.draw_stack == 0 {
                self.draw_stack += count * 2;
            } else {
                self.draw_stack += count;
            }
        } else {
            self.draw_stack = 0;
        }

        println!("Draw-stack is now the size of {}", self.draw_stack);

        self.placed_deck
            .push_front(p.cards.get(index).unwrap().clone());
        p.cards.remove(index);
    }

    pub fn switch_color(&mut self, color: Color) {
        let allowed_types = vec![Type::DrawFour, Type::Switch];

        if allowed_types.contains(&self.placed_deck.get(0).unwrap().r#type) {
            println!("{:#?}", &color);
            let c = self.placed_deck.get(0).unwrap().clone();

            self.broadcast(&MessagePacket::to_json(MessagePacket::new(&format!(
                "Switched color to {}",
                color
            ))));

            self.placed_deck
                .insert(0, Card::new_with_owner(c.r#type, color, c.owner));

            println!("{:#?}", self.placed_deck.get(0));
        }
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
    Zero,
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
            Type::Zero,
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
            Type::DrawTwo,
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

    fn new_with_owner(r#type: Type, color: Color, owner: Option<Uuid>) -> Card {
        Card {
            r#type,
            color,
            owner,
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
        let draw_cards = [Type::DrawTwo, Type::DrawFour];

        for card in deck {
            if last_card.owner == Some(owner) && last_card.owner.is_some() {
                // SAME TYPES
                if card.r#type == last_card.r#type {
                    l.push(card);
                    continue;
                }
            } else {
                // LAST CARD WAS A DRAW CARD PLACED BY ANOTHER "PLAYER"
                if draw_cards.contains(&last_card.r#type) && last_card.owner.is_some() {
                    // SPECIAL CARDS
                    if last_card.r#type == card.r#type {
                        l.push(card);
                        continue;
                    }
                } else {
                    // SPECIAL CARDS
                    if special.contains(&card.r#type) {
                        l.push(card);
                        continue;
                    }

                    // SAME COLORED CARDS
                    if card.color == last_card.color {
                        l.push(card);
                        continue;
                    }

                    // SAME TYPES
                    if card.r#type == last_card.r#type {
                        l.push(card);
                        continue;
                    }
                }
            }
        }

        l
    }
}
