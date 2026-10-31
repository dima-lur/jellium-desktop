use serde::{Deserialize, Serialize};

/// The instance protocol has exactly one request and uses compact JSON + LF.
pub const REQUEST_FRAME_BYTES: usize = b"{\"message\":\"Ping\"}\n".len();
pub const RESPONSE_FRAME_BYTES: usize = b"{\"message\":\"Pong\"}\n".len();

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "message")]
pub enum Request {
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "message")]
pub enum Response {
    Pong,
}

pub fn handle(req: &Request) -> Response {
    match req {
        Request::Ping => Response::Pong,
    }
}
