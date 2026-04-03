pub mod commands;
pub mod models;
pub mod utils;
pub mod systems;

use std::sync::Mutex;
use crate::models::SnapRect;

#[derive(Default)]
pub struct WindowState {
    pub maximize_button_rect: Mutex<Option<SnapRect>>,
}

// Global state or managers can be initialized here or in main.rs
