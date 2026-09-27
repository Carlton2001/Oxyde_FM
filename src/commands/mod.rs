pub mod archive;
pub mod config;
pub mod duplicates;
pub mod io;
pub mod ops;
pub mod search;
pub mod session;
pub mod sidebar;
pub mod system;
pub mod thumbnails;

#[cfg(target_os = "windows")]
pub use crate::platform::clipboard;
#[cfg(target_os = "windows")]
pub use crate::platform::icons;
#[cfg(target_os = "windows")]
pub use crate::platform::network;
