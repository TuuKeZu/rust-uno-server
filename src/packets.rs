use serde_json::Result;
use serde::{Serialize, Deserialize};

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