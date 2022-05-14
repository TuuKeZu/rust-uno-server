use crate::game::{Card, Color};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, strum_macros::Display)]
#[serde(tag = "type", content = "data")]
pub enum PacketType {
    Register(String),                              // username
    GameData(Uuid, String, Vec<(Uuid, String)>),   // self_id, self_username, Vec<(id, username)>
    Connect(Uuid, String),                         // id, username
    Disconnect(Uuid, String),                      // id, username
    Message(String),                               // content
    StartGame(String),                             // option
    StatusUpdatePublic(Uuid, String, usize, Card), // id, username, card-count, current
    StatusUpdatePrivate(Vec<Card>, Card),          // cards, current
    AllowedCardsUpdate(Vec<Card>),                 // allowed-cards
    DrawCard(u8),                                  // amount
    PlaceCard(usize),                              // index
    EndTurn,                                       //
    ColorSwitch(Color),                            // color
    TurnUpdate(Uuid, Uuid),                        // current, next
    Error(u64, String),                            // error-code, body
}
