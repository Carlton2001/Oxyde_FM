pub mod archive;
pub mod io;
pub mod ops;
pub mod search;
pub mod session;
pub mod config;
pub mod sidebar;
pub mod system;
pub mod thumbnails;
pub mod duplicates;

#[cfg(target_os = "windows")]
pub mod clipboard;
#[cfg(target_os = "windows")]
pub mod icons;
#[cfg(target_os = "windows")]
pub mod network;
