use crate::game::Card;
use serde::{Deserialize, Serialize};
use serde_json::Result;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterPacket {
    pub r#type: String,
    pub username: String,
}

impl RegisterPacket {
    pub fn try_parse(data: &str) -> Result<RegisterPacket> {
        serde_json::from_str(data)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessagePacket {
    pub r#type: String,
    pub content: String,
}

impl MessagePacket {
    pub fn try_parse(data: &str) -> Result<MessagePacket> {
        serde_json::from_str(data)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StartPacket {
    pub r#type: String,
    pub options: String,
}

impl StartPacket {
    pub fn try_parse(data: &str) -> Result<StartPacket> {
        serde_json::from_str(data)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicGamePacket {
    pub r#type: String,
    pub id: Uuid,
    pub cards: usize,
}

impl PublicGamePacket {
    pub fn new(id: Uuid, cards: usize) -> PublicGamePacket {
        PublicGamePacket {
            r#type: String::from("STATUS-UPDATE-PUBLIC"),
            id,
            cards,
        }
    }

    pub fn to_json(data: PublicGamePacket) -> String {
        serde_json::to_string(&data).unwrap()
    }

    pub fn try_parse(data: &str) -> Result<PublicGamePacket> {
        serde_json::from_str(data)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrivateGamePacket {
    pub r#type: String,
    pub cards: Vec<Card>,
}

impl PrivateGamePacket {
    pub fn new(cards: Vec<Card>) -> PrivateGamePacket {
        PrivateGamePacket {
            r#type: String::from("STATUS-UPDATE-PRIVATE"),
            cards,
        }
    }

    pub fn to_json(data: PrivateGamePacket) -> String {
        serde_json::to_string(&data).unwrap()
    }

    pub fn try_parse(data: &str) -> Result<PrivateGamePacket> {
        serde_json::from_str(data)
    }
}
