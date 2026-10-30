//! Headless self-worker entry. No GPUI app, preferences or application logs.
use oxyde_lib::utils::preview_protocol::{self, Reply};
use std::io::{Read, Write};

pub(crate) fn run_if_requested() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--oxyde-preview-worker-v1")) {
        return false;
    }
    // Do not even open the document until the parent has attached our Windows job.
    let mut gate = [0];
    let result = (|| -> std::io::Result<()> {
        std::io::stdin().read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(std::io::Error::other("Invalid preview start gate"));
        }
        oxyde_lib::platform::platform().initialize_preview_worker()?;
        let mode = arguments
            .next()
            .and_then(|mode| mode.into_string().ok())
            .ok_or_else(|| std::io::Error::other("Missing preview mode"))?;
        if mode == "pdf-previews" {
            if arguments.next().is_some() { return Err(std::io::Error::other("Unexpected PDF session argument")); }
            return oxyde_lib::platform::platform().run_pdf_preview_session_worker();
        }
        if mode == "media-registry" {
            if arguments.next().is_some() { return Err(std::io::Error::other("Unexpected registry argument")); }
            return oxyde_lib::platform::platform().run_media_registry_worker();
        }
        let path = arguments
            .next()
            .ok_or_else(|| std::io::Error::other("Missing preview source"))?;
        if arguments.next().is_some() {
            return Err(std::io::Error::other("Unexpected preview argument"));
        }
        let path = std::path::Path::new(&path);
        if mode == "viewer-media" {
            return oxyde_lib::platform::platform().run_viewer_media_worker(path);
        }
        if mode == "pdf-session" {
            return oxyde_lib::platform::platform().run_pdf_session_worker(path);
        }
        if mode == "video" {
            let path = path
                .to_str()
                .ok_or_else(|| std::io::Error::other("Video path is not Unicode"))?
                .to_owned();
            let mut output = std::io::stdout().lock();
            let mut send_error = None;
            let result = oxyde_lib::platform::local_video_preview_worker(path, |image, pts| {
                let reply = Reply::Images {
                    frames: vec![image::Frame::new(image)],
                    limited: false,
                    natural_size: None,
                };
                match video_packet(reply, pts, &mut output) {
                    Ok(()) => true,
                    Err(error) => {
                        send_error = Some(error);
                        false
                    }
                }
            });
            if let Some(error) = send_error {
                return Err(error);
            }
            if let Err(error) = result {
                video_packet(
                    Reply::Error {
                        limit: false,
                        detail: error.to_string(),
                    },
                    std::time::Duration::ZERO,
                    &mut output,
                )?;
            }
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        if mode == "image" {
            let reply = match super::image_preview::load_path_local(
                path,
                &gpui::SvgRenderer::new(std::sync::Arc::new(())),
            ) {
                Ok(preview) => {
                    let mut frames = Vec::new();
                    for index in 0..preview.image.frame_count() {
                        let size = preview.image.size(index);
                        let pixels = preview
                            .image
                            .as_bytes(index)
                            .ok_or_else(|| std::io::Error::other("Missing preview pixels"))?;
                        let image = image::RgbaImage::from_raw(
                            size.width.0 as u32,
                            size.height.0 as u32,
                            pixels.to_vec(),
                        )
                        .ok_or_else(|| std::io::Error::other("Invalid preview pixels"))?;
                        frames.push(image::Frame::from_parts(
                            image,
                            0,
                            0,
                            preview.image.delay(index),
                        ));
                    }
                    Reply::Images {
                        frames,
                        limited: preview.notice.is_some(),
                        natural_size: preview.natural_size,
                    }
                }
                Err(error) => Reply::Error {
                    limit: matches!(error, super::image_preview::PreviewError::Limit(_)),
                    detail: error.to_string(),
                },
            };
            return preview_protocol::encode(reply, &mut std::io::stdout().lock());
        }
        #[cfg(not(target_os = "windows"))]
        {
            if mode == "audio-metadata" {
                let metadata = oxyde_lib::utils::audio_metadata::read(std::fs::File::open(path)?)?;
                let mut output = std::io::stdout().lock();
                serde_json::to_writer(&mut output, &metadata).map_err(std::io::Error::other)?;
                return output.flush();
            }
            if let Some(reply) = super::image_preview::worker_reply(&mode, path) {
                return preview_protocol::encode(reply, &mut std::io::stdout().lock());
            }
        }
        let reply = oxyde_lib::platform::local_preview_worker_reply(&mode, path);
        preview_protocol::encode(reply, &mut std::io::stdout().lock())
    })();
    if let Err(error) = result {
        let _ = writeln!(std::io::stderr(), "Preview worker failed: {error}");
    }
    // Native PDF/GPU DLL teardown may hang even after producing its reply.
    // stdout has already been flushed, and this worker has no application state.
    let _ = oxyde_lib::platform::platform().finish_process_shutdown();
    true
}

fn video_packet(
    reply: Reply,
    timestamp: std::time::Duration,
    output: &mut impl Write,
) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    preview_protocol::encode(reply, &mut bytes)?;
    let length = bytes.len() + 12;
    if length > 2 * 1024 * 1024 {
        return Err(std::io::Error::other("Video packet exceeds its limit"));
    }
    output.write_all(&(length as u32).to_le_bytes())?;
    output.write_all(&timestamp.as_secs().to_le_bytes())?;
    output.write_all(&timestamp.subsec_nanos().to_le_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}
