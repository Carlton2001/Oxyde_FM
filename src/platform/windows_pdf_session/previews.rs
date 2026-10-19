//! Native PDF thumbnails in a bounded, sequential path session; no GPUI state.
use crate::{
    commands::icon_session_worker::MAX_INPUT,
    utils::preview_protocol::{self, Reply},
};
use std::io::{self, Read, Write};

pub(super) fn run() -> io::Result<()> {
    let _com =
        crate::platform::windows_com::ComApartment::mta_or_existing().map_err(io::Error::other)?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut consumed = 0;
    for id in 1u64..=32 {
        let mut header = [0u8; 5];
        match input.read_exact(&mut header) {
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            result => result?,
        }
        let length = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
        if header[0] > 1 || length == 0 || length > MAX_INPUT.saturating_sub(consumed + 5) {
            return Err(io::Error::other("Invalid PDF preview request"));
        }
        consumed += 5 + length;
        let mut path = vec![0; length];
        input.read_exact(&mut path)?;
        let path = String::from_utf8(path).map_err(io::Error::other)?;
        let image = if header[0] == 0 {
            crate::platform::windows_pdf_thumbnail::get_grid(path)
        } else {
            crate::platform::windows_pdf_thumbnail::get(path)
        };
        let failed = image.is_err();
        let reply = match image {
            Ok(image) => Reply::Images {
                frames: vec![image::Frame::new(image)],
                limited: false,
                natural_size: None,
            },
            Err(error) => Reply::Error {
                limit: false,
                detail: error.to_string(),
            },
        };
        let mut bytes = Vec::new();
        preview_protocol::encode(reply, &mut bytes)?;
        output.write_all(&id.to_le_bytes())?;
        output.write_all(&(bytes.len() as u32).to_le_bytes())?;
        output.write_all(&bytes)?;
        output.flush()?;
        if failed {
            return Ok(());
        }
    }
    Ok(())
}
