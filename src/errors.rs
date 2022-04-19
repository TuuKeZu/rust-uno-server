use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct HTMLError {
    pub r#type: String,
    pub status_code: u64,
    pub body: String,
}

impl HTMLError {
    pub fn to_json(err: HTMLError) -> String {
        serde_json::to_string(&err).unwrap()
    }

    pub fn new(status_code: u64, body: &str) -> HTMLError {
        HTMLError {
            r#type: String::from("ERROR"),
            status_code,
            body: String::from(body),
        }
    }
}
