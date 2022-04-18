use serde_json::Result;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct RegisterPacket {
    pub r#type: String,
    pub username: String
}

impl RegisterPacket {
    pub fn try_parse(data: &str) -> Result<RegisterPacket> {
        serde_json::from_str(data)
    }
}


#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct MessagePacket {
    pub r#type: String,
    pub content: String
}

impl MessagePacket {
    pub fn try_parse(data: &str) -> Result<MessagePacket> {
        serde_json::from_str(data)
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct StartPacket {
    pub r#type: String,
    pub options: String
}

impl StartPacket {
    pub fn try_parse(data: &str) -> Result<StartPacket> {
        serde_json::from_str(data)
    }
}
