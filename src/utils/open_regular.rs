//! Opening a path that must be an ordinary file, without blocking on a named pipe.
use std::fs::File;
use std::io;
use std::path::Path;

/// Opens `path` for reading and refuses anything but a regular file. On Unix the open
/// does not wait: a FIFO without a writer would otherwise block it for ever.
pub fn open_regular(path: &Path) -> io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Not a regular file"));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::open_regular;

    #[test]
    fn a_regular_file_opens_and_a_directory_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(open_regular(&file).is_ok());
        assert!(open_regular(temp.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_named_pipe_without_writer_is_refused_instead_of_blocking() {
        let temp = tempfile::tempdir().unwrap();
        let pipe = temp.path().join("p");
        let name = std::ffi::CString::new(pipe.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(open_regular(&pipe).is_err());
    }
}
