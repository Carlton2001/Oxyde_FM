//! Bounded, sequential Shell icon requests. No document preview parsers.
use crate::utils::preview_protocol;
use std::io::{self, Read, Write};

pub const FLAG: &str = "--oxyde-icon-session-worker-v1";
pub const MAX_INPUT: usize = 4095; // At most the capacity of the parent's stdin pipe, minus its gate.
pub const MAX_REPLY: usize = 1024 * 1024;
pub const MAX_REQUESTS: u64 = 64;

pub fn run_if_requested() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    let result = serve(&mut io::stdin().lock(), &mut io::stdout().lock());
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "Icon session failed: {error}");
    }
    let _ = crate::platform::platform().finish_process_shutdown();
    true
}

fn serve(input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
    let mut gate = [0];
    input.read_exact(&mut gate)?;
    if gate != [0xA7] {
        return Err(io::Error::other("Invalid icon session gate"));
    }
    crate::platform::platform().initialize_preview_worker()?;
    let mut consumed = 0;
    for id in 1..=MAX_REQUESTS {
        let mut header = [0; 5];
        if input.read(&mut header[..1])? == 0 {
            return Ok(());
        }
        input.read_exact(&mut header[1..])?;
        let length = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
        if header[0] > 1 || length == 0 || length > MAX_INPUT.saturating_sub(consumed + 5) {
            return Err(io::Error::other("Invalid icon session request"));
        }
        consumed += 5 + length;
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes)?;
        let path = String::from_utf8(bytes).map_err(io::Error::other)?;
        let mode = if header[0] == 0 {
            "icon-small"
        } else {
            "icon-large"
        };
        let reply = crate::platform::local_preview_worker_reply(mode, std::path::Path::new(&path));
        let mut encoded = Vec::new();
        preview_protocol::encode(reply, &mut encoded)?;
        if encoded.len() > MAX_REPLY {
            return Err(io::Error::other("Icon session reply too large"));
        }
        output.write_all(&id.to_le_bytes())?;
        output.write_all(&(encoded.len() as u32).to_le_bytes())?;
        output.write_all(&encoded)?;
        output.flush()?;
    }
    Ok(())
}
