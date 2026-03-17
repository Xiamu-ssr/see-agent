use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// RPC method constants
pub mod methods {
    pub const BUS_SEND: &str = "bus.send";
    pub const BUS_DRAIN: &str = "bus.drain";
    pub const BOARD_LIST: &str = "board.list";
    pub const BOARD_CREATE: &str = "board.create";
    pub const BOARD_CLAIM: &str = "board.claim";
    pub const BOARD_COMPLETE: &str = "board.complete";
    pub const BOARD_UPDATE: &str = "board.update";
    pub const BOARD_ASSIGN: &str = "board.assign";
    pub const SCREEN_ACQUIRE: &str = "screen.acquire";
    pub const SCREEN_RELEASE: &str = "screen.release";
    pub const SCREEN_CAPTURE: &str = "screen.capture";
}
