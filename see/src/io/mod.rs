mod atomic;
mod json;
mod jsonl;
mod markdown;

pub use atomic::atomic_write;
pub use json::{read_json, write_json};
pub use jsonl::{append_jsonl, read_jsonl};
pub use markdown::{read_text, write_text};
