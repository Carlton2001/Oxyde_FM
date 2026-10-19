//! Sequential requests carry read-only handles, never paths or parent authority.
use super::*;

pub(super) fn run(
    svg: &dyn Fn(
        &[u8],
        image_preview_decode::Policy,
    ) -> Result<Vec<image::Frame>, image_preview_decode::PreviewError>,
) -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut gate = [0];
    input.read_exact(&mut gate)?;
    if gate != [0xA7] {
        return Err(io::Error::other("Invalid image start gate"));
    }
    let mut covers = crate::utils::audio_metadata::CoverReader::default();
    for expected in 1..=SESSION_REQUESTS {
        let mut request = [0; 17];
        input.read_exact(&mut request)?;
        let id = u64::from_le_bytes(request[..8].try_into().unwrap());
        let handle = usize::try_from(u64::from_le_bytes(request[8..16].try_into().unwrap()))
            .map_err(io::Error::other)?;
        if id != expected || handle == 0 || handle == usize::MAX || request[16] > 4 {
            return Err(io::Error::other("Invalid image session request"));
        }
        let policy = match request[16] {
            1 => image_preview_decode::Policy {
                output_side: 128,
                frame_count: 1,
                ..POLICY
            },
            2 => image_preview_decode::VIEWER_POLICY,
            _ => POLICY,
        };
        let file = unsafe { File::from_raw_handle(handle as *mut std::ffi::c_void) };
        if request[16] == 3 || request[16] == 4 {
            let reply = match covers.read(file, request[16] == 4) {
                Ok(Some(cover)) => {
                    let image = image::RgbaImage::from_raw(cover.width, cover.height, cover.pixels)
                        .ok_or_else(|| io::Error::other("Invalid cover pixels"))?;
                    Reply::Images { frames: vec![image::Frame::new(image)], limited: false, natural_size: None }
                }
                Ok(None) => Reply::Text(String::new()),
                Err(error) => Reply::Error { limit: false, detail: error.to_string() },
            };
            send_reply(&mut output, id, reply)?;
            continue;
        }
        // read_bounded consumes and closes this request's source before parsing.
        let decoded = image_preview_decode::read_bounded(file, policy.encoded_bytes)
            .and_then(|data| image_preview_decode::decode(&data, policy, svg));
        let reply = match decoded {
            Ok(decoded) => Reply::Images {
                frames: decoded.frames,
                limited: decoded.limited,
                natural_size: decoded.natural_size,
            },
            Err(error) => Reply::Error {
                limit: matches!(error, image_preview_decode::PreviewError::Limit(_)),
                detail: error.to_string(),
            },
        };
        send_reply(&mut output, id, reply)?;
    }
    Ok(())
}

fn send_reply(output: &mut impl Write, id: u64, reply: Reply) -> io::Result<()> {
    let mut bytes = Vec::new();
    preview_protocol::encode(reply, &mut bytes)?;
    output.write_all(&id.to_le_bytes())?;
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}
