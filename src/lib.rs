// MSVC link.exe prints expected import-library/export-object creation notices to stdout.
#![allow(linker_messages)]

pub mod commands;
pub mod models;
pub mod platform;
pub mod systems;
pub mod utils;

use crate::models::SnapRect;
use std::sync::Mutex;

#[derive(Default)]
pub struct WindowState {
    pub maximize_button_rect: Mutex<Option<SnapRect>>,
}

// Global state or managers can be initialized here or in main.rs
