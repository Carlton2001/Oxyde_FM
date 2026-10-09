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
        let path = arguments
            .next()
            .ok_or_else(|| std::io::Error::other("Missing preview source"))?;
        if arguments.next().is_some() {
            return Err(std::io::Error::other("Unexpected preview argument"));
        }
        let path = std::path::Path::new(&path);
        let reply = if mode == "image" {
            match super::image_preview::load_path_local(
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
                    }
                }
                Err(error) => Reply::Error {
                    limit: matches!(error, super::image_preview::PreviewError::Limit(_)),
                    detail: error.to_string(),
                },
            }
        } else {
            oxyde_lib::platform::local_preview_worker_reply(&mode, path)
        };
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
