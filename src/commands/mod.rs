pub mod archive;
#[doc(hidden)]
pub mod archive_io_codec;
#[doc(hidden)]
pub mod archive_io_worker;
#[doc(hidden)]
pub mod archive_worker;
#[doc(hidden)]
#[cfg(target_os = "windows")]
pub mod archive_handle_worker;
#[doc(hidden)]
#[cfg(target_os = "windows")]
pub mod image_handle_worker;
#[doc(hidden)]
#[cfg(target_os = "windows")]
pub mod office_handle_worker;
#[doc(hidden)]
#[cfg(target_os = "windows")]
pub mod audio_handle_worker;
mod archive_iso_open;
mod archive_metadata;
mod archive_seven_open;
#[cfg(test)]
mod archive_tar_open;
pub mod duplicates;
pub mod io;
pub mod ops;
pub mod search;
pub mod sidebar;
pub mod system;
#[doc(hidden)]
#[cfg(target_os = "windows")]
pub mod icon_session_worker;
