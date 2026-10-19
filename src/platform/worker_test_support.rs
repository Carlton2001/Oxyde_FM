//! Narrow integration-test entry points into the compiled native supervisors.
//! Keep implementation modules private and exercise the same launch locks,
//! resource budgets and LPAC transport as the application. No local decoders.
use std::{
    io,
    process::{Child, Command},
};

/// Compile native test probes through the production handle-inheritance lock.
pub fn spawn(command: Command) -> io::Result<Child> {
    super::windows_process_launch::spawn(command)
}

pub mod preview {
    use super::super::windows_preview_worker as native;
    use std::{fs::File, io, path::Path, process::Command, time::Duration};

    pub fn run_in(
        executable: &Path,
        mode: &str,
        path: &Path,
        deadline: Duration,
    ) -> io::Result<Vec<u8>> {
        native::run_in(executable, mode, path, deadline)
    }
    pub fn run_image_in(executable: &Path, path: &Path, deadline: Duration) -> io::Result<Vec<u8>> {
        native::run_image_mode_in(executable, path, deadline, false)
    }
    pub fn run_office_text_in(
        executable: &Path,
        path: &Path,
        deadline: Duration,
    ) -> io::Result<Vec<u8>> {
        native::run_office_text_in(executable, path, deadline, false)
    }
    pub fn supervise(command: Command, deadline: Duration) -> io::Result<Vec<u8>> {
        native::supervise(command, deadline)
    }
    pub fn supervise_source(
        command: Command,
        source: &File,
        deadline: Duration,
        memory: usize,
        max_reply: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        native::supervise_source(command, source, deadline, memory, max_reply, checkpoint)
    }
    pub fn stream_in(
        executable: &Path,
        path: &Path,
        on_frame: &mut dyn FnMut(image::RgbaImage, Duration) -> bool,
        cancelled: &dyn Fn() -> bool,
        deadline: Duration,
    ) -> io::Result<()> {
        native::stream_in(executable, path, on_frame, cancelled, deadline)
    }
    pub fn supervise_stream(
        command: Command,
        on_frame: &mut dyn FnMut(image::RgbaImage, Duration) -> bool,
        cancelled: &dyn Fn() -> bool,
        deadline: Duration,
    ) -> io::Result<()> {
        native::supervise_stream(command, on_frame, cancelled, deadline)
    }
}

pub mod archive_listing {
    use super::super::windows_archive_worker as native;
    use crate::models::{CommandError, FileEntry};
    use std::{io, path::Path, process::Command, time::Duration};

    pub fn run_in(
        executable: &Path,
        path: &Path,
        internal: &str,
        deadline: Duration,
    ) -> Result<Vec<FileEntry>, CommandError> {
        native::run_in(executable, path, internal, deadline)
    }
    pub fn index_in(
        executable: &Path,
        path: &Path,
        deadline: Duration,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> Result<Vec<FileEntry>, CommandError> {
        native::index_in(executable, path, deadline, checkpoint)
    }
    pub fn supervise(
        command: Command,
        path: &Path,
        internal: &str,
        deadline: Duration,
    ) -> Result<Vec<FileEntry>, CommandError> {
        native::supervise(command, path, internal, deadline)
    }
    pub fn supervise_index(
        command: Command,
        path: &Path,
        deadline: Duration,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> Result<Vec<FileEntry>, CommandError> {
        native::supervise_index(command, path, deadline, checkpoint)
    }
}

pub mod archive_io {
    use super::super::windows_archive_io_worker as native;
    use crate::utils::archive_io_protocol::{Control, Frame, Operation};
    use std::{io, path::Path, process::Command, time::Duration};

    pub struct Stats {
        pub peak_memory: usize,
        pub active: Duration,
    }
    impl From<native::Stats> for Stats {
        fn from(stats: native::Stats) -> Self {
            Self {
                peak_memory: stats.peak_memory,
                active: stats.active,
            }
        }
    }
    pub fn run_in(
        executable: &Path,
        operation: Operation,
        controls: &dyn Fn() -> io::Result<Control>,
        handler: &mut dyn FnMut(Frame) -> io::Result<Frame>,
        deadline: Duration,
    ) -> io::Result<Stats> {
        native::run_in(executable, operation, controls, handler, deadline).map(Into::into)
    }
    pub fn supervise(
        command: Command,
        session: [u8; 16],
        operation: Operation,
        controls: &dyn Fn() -> io::Result<Control>,
        handler: &mut dyn FnMut(Frame) -> io::Result<Frame>,
        deadline: Duration,
        memory: usize,
    ) -> io::Result<Stats> {
        native::supervise(
            command, session, operation, controls, handler, deadline, memory,
        )
        .map(Into::into)
    }
}
