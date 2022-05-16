use crate::messages::WsMessage;
use crate::packets::*;
use actix::prelude::Recipient;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;
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
    pub block_stack: usize,
    pub reversed: bool,

    pub statistics: GameStatistics,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameStatistics {
    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
    pub player_count: usize,
    pub spectator_count: usize,
    pub cards_placed: usize,
    pub cards_drawn: usize,
}

impl GameStatistics {
    pub fn new() -> GameStatistics {
        GameStatistics {
            start_time: None,
            end_time: None,
            player_count: 0,
            spectator_count: 0,
            cards_placed: 0,
            cards_drawn: 0,
        }
    }

    pub fn game_started(&mut self) {
        self.start_time = Some(SystemTime::now());
    }

    pub fn game_ended(&mut self) {
        self.end_time = Some(SystemTime::now());
    }

    pub fn card_placed(&mut self) {
        self.cards_placed += 1;
    }

    pub fn card_drawn(&mut self) {
        self.cards_drawn += 1;
    }
}

impl Default for GameStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct Players(VecDeque<(Uuid, Player)>);

impl Players {
    // Returns immutable list of keys representing players
    pub fn keys(&self) -> Vec<&Uuid> {
        self.0.iter().map(|pair| &pair.0).collect::<Vec<&Uuid>>()
    }
    // Returns immutable list of players
    pub fn players(&self) -> Vec<&Player> {
        self.0.iter().map(|pair| &pair.1).collect::<Vec<&Player>>()
    }
    // Returns mutable list of keys representing players
    pub fn keys_mut(&self) -> Vec<Uuid> {
        self.0.iter().map(|pair| pair.0).collect::<Vec<Uuid>>()
    }
    // Returns the number of players in game
    pub fn len(&self) -> usize {
        self.0.len()
    }
    // Returns a boolean indicating weather there is players in the game
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    // Returns a list of tuples containing the uuid of the player and it's username
    pub fn map_username(&self) -> Vec<(Uuid, String)> {
        self.0
            .iter()
            .map(|(k, p)| (*k, p.username.clone()))
            .collect()
    }
    // Returns a list of players ordered by the number of cards they have (asc)
    pub fn sort_by_cards(&self) -> VecDeque<&Player> {
        let mut players = self.players().clone();
        players.sort_by_key(|p| p.cards.len());
        players.into()
    }
    // Returns immutable player with the uuid given as argument
    pub fn get(&self, key: &Uuid) -> Option<&Player> {
        let x = self
            .0
            .iter()
            .find(|pair| pair.0 == *key)
            .map(|pair| &pair.1);
        x
    }
    // Returns mutable player with the uuid given as argument
    pub fn get_mut(&mut self, key: &Uuid) -> Option<&mut Player> {
        let x = self
            .0
            .iter_mut()
            .find(|pair| pair.0 == *key)
            .map(|pair| &mut pair.1);
        x
    }
    // Returns a boolean indicating weather there's a player with given uuid
    pub fn contains_key(&self, key: &Uuid) -> bool {
        self.keys().contains(&key)
    }
    // Removes the player with given uuid
    pub fn remove(&mut self, key: &Uuid) {
        // l
        self.0
            .remove(self.0.iter().position(|pair| pair.0 == *key).unwrap());
    }
    // Inserts a uuid-player pair
    pub fn insert(&mut self, key: Uuid, player: Player) {
        self.0.push_back((key, player));
    }
    // Rotates the players list and returns the uuid of the current player.
    pub fn next_player(&mut self, reversed: bool) -> Uuid {
        if !reversed {
            let current = self.0.pop_back().unwrap();
            self.0.push_front(current.clone());
            current.0
        } else {
            let current = self.0.pop_front().unwrap();
            self.0.push_back(current.clone());
            current.0
        }
    }
    // Predicts which player would be next if 'next_player' would be called.
    pub fn predict_next(&self, reversed: bool) -> Uuid {
        let mut players = self.0.clone();
        if !reversed {
            let current = players.pop_back().unwrap();
            players.push_front(current.clone());
            current.0
        } else {
            let current = players.pop_front().unwrap();
            players.push_back(current.clone());
            current.0
        }
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
            block_stack: 0,
            reversed: false,
            statistics: GameStatistics::default(),
        }
    }

    pub fn leave(&mut self, id: Uuid) {
        if self.players.contains_key(&id) {
            self.players.remove(&id);
        } else if self.spectators.contains_key(&id) {
            self.spectators.remove(&id);
        } else {
            return;
        }

        if self.active {
            self.broadcast(&to_json(PacketType::Message(
                "Server".to_string(),
                "Game ended due to one of the players leaving".to_string(),
            )));
            self.end();
        }
    }

    pub fn get_player(&mut self, id: &Uuid) -> &Player {
        self.players.get(id).unwrap()
    }

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

    pub fn broadcast_ignore_self(&self, self_id: Uuid, data: &str) {
        for id in self.players.keys() {
            if &self_id != id {
                self.send_message(data, id);
            }
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
                &to_json(PacketType::Message(
                    "Server".to_string(),
                    "You are the host".to_string(),
                )),
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
        self.statistics.game_started();

        self.broadcast(&to_json(PacketType::Message(
            "Server".to_string(),
            "The host has started the game".to_string(),
        )));
    }

    pub fn end(&mut self) {
        println!("Player won the game");
        self.statistics.game_ended();
        self.statistics.player_count = self.players.len();
        let mut placements = self.players.sort_by_cards();
        let winner = placements.pop_front().unwrap();

        let p = PacketType::WinUpdate(
            winner.id,
            winner.username.clone(),
            placements.iter().map(|p| p.username.clone()).collect(),
            self.statistics.clone(),
        );

        self.broadcast(&to_json(p));

        self.active = false;
    }

    pub fn give_turn(&mut self) {
        let current = self.next_turn();

        self.emit(
            &current,
            &to_json(PacketType::Message(
                "Server".to_string(),
                "Your turn".to_string(),
            )),
        );

        self.broadcast(&to_json(PacketType::TurnUpdate(
            current,
            self.players.predict_next(self.reversed),
        )));

        self.update_allowed_status(&current);
    }

    pub fn end_turn(&mut self, id: Uuid) {
        let draw_cards = [Type::DrawTwo, Type::DrawFour];
        let last_card = self.placed_deck.get(0).unwrap();

        // Check if the player can end their turn => allow in the case of the last card was a draw-card
        if !(self
            .players
            .get(&self.current_turn.unwrap())
            .unwrap()
            .can_end()
            || draw_cards.contains(&last_card.r#type) && last_card.owner.is_some())
        {
            self.emit(
                &id,
                &to_json(PacketType::Error(
                    401,
                    "Cannot end your turn yet. Please either place a card or draw three cards"
                        .to_string(),
                )),
            );
            return;
        }

        // Drawing cards
        if last_card.owner != self.current_turn
            && last_card.owner.is_some()
            && draw_cards.contains(&last_card.r#type)
        {
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
        }

        // Last card will always be owned by the last person who placed it
        if self.placed_deck.get(0).unwrap().owner.is_some() {
            self.placed_deck.get_mut(0).unwrap().owner = self.current_turn;
        }

        // Reversing
        if self.placed_deck.get(0).unwrap().r#type == Type::Reverse {
            self.reversed = !self.reversed;

            // Only give the turn back to the player if there's less than 3 players
            if self.players.len() > 2 {
                self.players.next_player(self.reversed);
            }

            self.placed_deck.get_mut(0).unwrap().owner = None;
        }

        // Blocking
        if self.placed_deck.get(0).unwrap().r#type == Type::Block {
            let count = if self.block_stack > 1 {
                self.block_stack
            } else {
                1
            };

            for _ in 0..count {
                self.players.next_player(self.reversed);
            }
            // Reset block-stack and allow the same player to place cards by deowning the block-card.
            self.placed_deck.get_mut(0).unwrap().owner = None;
            self.block_stack = 0;
        }
        // Clear all the actions done by the player during this turn
        self.players
            .get_mut(&self.current_turn.unwrap())
            .unwrap()
            .actions
            .clear();

        // Send back the 'EndTurnPacket' to client to indicate their turn has ended
        self.emit(&self.current_turn.unwrap(), &to_json(PacketType::EndTurn));

        // Update the status
        self.update_card_status(&self.current_turn.unwrap());

        // There is no cards left => Player wins
        if self
            .players
            .get_mut(&self.current_turn.unwrap())
            .unwrap()
            .cards
            .is_empty()
        {
            self.end();
            return;
        }

        // Continue the game as normal
        self.give_turn();
    }

    pub fn update_card_status(&self, self_id: &Uuid) {
        for id in self.players.keys() {
            if self_id == id {
                /*
                let p: PrivateGamePacket = PrivateGamePacket::new(
                    self.players.get(id).unwrap().cards.clone(),
                    self.placed_deck.get(0).unwrap().clone(),
                );
                */
                self.emit(
                    id,
                    &to_json(PacketType::StatusUpdatePrivate(
                        self.players.get(id).unwrap().cards.clone(),
                        self.placed_deck.get(0).unwrap().clone(),
                    )),
                );
            } else {
                /*
                let p: PublicGamePacket = PublicGamePacket::new(
                    self_id.to_owned(),
                    &self.players.get(self_id).unwrap().username,
                    self.players.get(self_id).unwrap().cards.len(),
                    self.placed_deck.get(0).unwrap().clone(),
                );
                */
                self.emit(
                    id,
                    &to_json(PacketType::StatusUpdatePublic(
                        self_id.to_owned(),
                        self.players.get(self_id).unwrap().username.clone(),
                        self.players.get(self_id).unwrap().cards.len(),
                        self.placed_deck.get(0).unwrap().clone(),
                    )),
                );
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

        self.send_message(&to_json(PacketType::AllowedCardsUpdate(allowed)), self_id);
    }

    pub fn draw_cards(&mut self, count: usize, owner: Uuid) {
        let mut l: Vec<Card> = Vec::new();
        let p = self.players.get_mut(&owner).unwrap();

        for _ in 0..count {
            if self.deck.is_empty() {
                self.deck.extend(Card::generate_deck());
            }

            // l.push(self.deck.pop_front().unwrap());
            l.push(Card::new(Type::Five, Color::Red));

            self.statistics.card_drawn();
        }

        l.iter_mut().for_each(|card| card.owner = Some(owner));
        p.cards.extend(l);

        // Puch the action to the actions list
        p.actions.push(Actions::DrawCard);
    }

    pub fn next_turn(&mut self) -> Uuid {
        self.current_turn = Some(self.players.next_player(self.reversed));
        self.current_turn.unwrap()
    }

    pub fn place_card(&mut self, index: usize, id: Uuid) {
        let draw_cards = [Type::DrawTwo, Type::DrawFour];
        let p = self.players.get(&id).unwrap();

        // Stacked draw-cards
        if self.placed_deck.get(0).unwrap().r#type == p.cards.get(index).unwrap().r#type
            && draw_cards.contains(&self.placed_deck.get(0).unwrap().r#type)
        {
            let count = if self.placed_deck.get(0).unwrap().r#type == Type::DrawFour {
                4
            } else {
                2
            };
            self.draw_stack += if self.draw_stack == 0 {
                count * 2
            } else {
                count
            }
        } else {
            self.draw_stack = 0;
        }

        // Stacked block-cards
        if self.placed_deck.get(0).unwrap().r#type == p.cards.get(index).unwrap().r#type
            && self.placed_deck.get(0).unwrap().r#type == Type::Block
            && p.cards.get(index).unwrap().owner == self.placed_deck.get(0).unwrap().owner
        {
            self.block_stack += if self.block_stack == 0 { 2 } else { 1 };
        } else {
            self.block_stack = 0;
        }
        println!("block stack is now the size of {}", self.block_stack);

        //Shadowing player now when we need it mutable
        let p = self.players.get_mut(&id).unwrap();

        // Puch the action to the actions list
        p.actions.push(Actions::PlaceCard);

        self.placed_deck
            .push_front(p.cards.get(index).unwrap().clone());
        p.cards.remove(index);

        self.statistics.card_placed();
    }

    pub fn switch_color(&mut self, color: Color) {
        let allowed_types = vec![Type::DrawFour, Type::Switch];

        if allowed_types.contains(&self.placed_deck.get(0).unwrap().r#type) {
            println!("{:#?}", &color);
            let c = self.placed_deck.get(0).unwrap().clone();

            self.broadcast(&to_json(PacketType::Message(
                "Server".to_string(),
                format!("Switched color to {}", color),
            )));

            self.placed_deck
                .insert(0, Card::new_with_owner(c.r#type, color, c.owner));

            println!("{:#?}", self.placed_deck.get(0));
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
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
    pub waiting: bool,
    actions: Vec<Actions>,
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
            waiting: false,
            actions: Vec::new(),
        }
    }

    pub fn can_end(&self) -> bool {
        // Player can end their turn only if they have placed one card or drawn 3 cards
        self.actions
            .iter()
            .filter(|a| **a == Actions::PlaceCard)
            .count()
            >= 1
            || self
                .actions
                .iter()
                .filter(|a| **a == Actions::DrawCard)
                .count()
                >= 3
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Actions {
    DrawCard,
    PlaceCard,
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
        /*
        deck.iter()
            .filter(|card| !disallowed_types.contains(&card.r#type))
            .collect::<VecDeque<&Card>>()
            .pop_back()
            .unwrap()
            .clone()
        */
        Card::new(Type::Five, Color::Red)
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
                if last_card.owner.is_none() {
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
                } else if draw_cards.contains(&last_card.r#type) && last_card.owner != Some(owner) {
                    // SPECIAL CARDS
                    if last_card.r#type == card.r#type {
                        l.push(card);
                        continue;
                    }
                } else if last_card.owner != Some(owner) {
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

pub fn to_json(data: PacketType) -> String {
    serde_json::to_string(&data).unwrap()
}
