mod api;
mod app;
mod layout;
mod pages;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn main() {
    mount_to_body(app::App);
}
