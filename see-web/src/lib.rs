#[allow(dead_code)]
mod api;
mod app;
mod layout;
mod pages;

use leptos::prelude::*;

pub fn main() {
    mount_to_body(app::App);
}
