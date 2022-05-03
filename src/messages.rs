use crate::errors::HTMLError;
use actix::prelude::{Message, Recipient};
use serde::{Deserialize, Serialize};
use serde_json::{Result, Value};
use uuid::Uuid;

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<WsMessage>,
    pub lobby_id: Uuid,
    pub self_id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub room_id: Uuid,
    pub id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
#[derive(Serialize, Deserialize)]
pub struct Packet {
    pub id: Uuid,
    pub data: String,
    pub json: Value,
    pub room_id: Uuid,
}

impl Packet {
    pub fn try_parse(data: &str) -> Value {
        let v: Result<Value> = serde_json::from_str(data);

        match v {
            Ok(v) => v,
            Err(e) => {
                serde_json::from_str(&HTMLError::to_json(HTMLError::new(400, &e.to_string())))
                    .unwrap()
            }
        }
    }

    pub fn new(id: Uuid, data: &str, room_id: Uuid) -> Packet {
        Packet {
            id,
            data: String::from(data),
            room_id,
            json: Packet::try_parse(data),
        }
    }
}
