use super::*;
use lofty::{
    config::WriteOptions,
    picture::{MimeType, Picture, PictureType},
    prelude::TagExt,
    tag::{ItemKey, Tag, TagType},
};
use std::io::Write;
use windows::Win32::System::{
    ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
    Threading::GetProcessId,
};

fn source(directory: &Path, name: &str, with_cover: bool) -> std::fs::File {
    let path = directory.join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x02\0\0\0\0\0").unwrap();
    drop(file);
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::TrackTitle, name.into());
    if with_cover {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(1024, 576, image::Rgba([200, 30, 10, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        tag.push_picture(
            Picture::unchecked(png.into_inner())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build(),
        );
    }
    tag.save_to(
        &mut std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap(),
        WriteOptions::default(),
    )
    .unwrap();
    std::fs::File::open(path).unwrap()
}

#[test]
fn persistent_audio_session_reuses_pid_for_grid_hover_and_missing_covers() {
    let directory = tempfile::tempdir().unwrap();
    let first = source(directory.path(), "first.wav", true);
    let second = source(directory.path(), "second.wav", true);
    let absent = source(directory.path(), "absent.wav", false);
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("oxyde-image-worker.exe");
    let cold = Instant::now();
    let mut session = Session::start_with(&executable, cold, AUDIO).unwrap();
    let process = HANDLE(session.worker.child.as_raw_handle());
    let pid = unsafe { GetProcessId(process) };
    let first_reply = session
        .request(&first, AUDIO_THUMBNAIL_KIND, Instant::now(), DEADLINE)
        .unwrap();
    let cold_time = cold.elapsed();
    assert!(first_reply.len() <= 128 * 128 * 4 + 128);
    let warm = Instant::now();
    for _ in 0..8 {
        // Production retains a freshly opened source for each request. Duplicated
        // Windows handles share their file cursor, so do the same in this probe.
        let fresh = std::fs::File::open(directory.path().join("second.wav")).unwrap();
        let reply = session
            .request(&fresh, AUDIO_THUMBNAIL_KIND, Instant::now(), DEADLINE)
            .unwrap();
        let Reply::Images { frames, .. } = preview_protocol::decode(&reply).unwrap() else {
            panic!("Expected thumbnail");
        };
        assert_eq!(frames[0].buffer().dimensions(), (128, 72));
        assert_eq!(frames[0].buffer().get_pixel(64, 36).0, [10, 30, 200, 255]);
        assert_eq!(unsafe { GetProcessId(process) }, pid);
    }
    let warm_time = warm.elapsed();
    let fresh = std::fs::File::open(directory.path().join("first.wav")).unwrap();
    let reply = session
        .request(&fresh, AUDIO_KIND, Instant::now(), DEADLINE)
        .unwrap();
    let Reply::Images { frames, .. } = preview_protocol::decode(&reply).unwrap() else {
        panic!("Expected hover cover");
    };
    assert_eq!(frames[0].buffer().dimensions(), (512, 288));
    let reply = session
        .request(&absent, AUDIO_THUMBNAIL_KIND, Instant::now(), DEADLINE)
        .unwrap();
    assert!(
        matches!(preview_protocol::decode(&reply).unwrap(), Reply::Text(text) if text.is_empty())
    );
    assert!(
        session.reusable(),
        "A valid file without artwork must not restart the process"
    );
    assert_eq!(unsafe { GetProcessId(process) }, pid);
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = std::mem::size_of_val(&counters) as u32;
    unsafe {
        GetProcessMemoryInfo(
            process,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            std::mem::size_of_val(&counters) as u32,
        )
    }
    .unwrap();
    let legacy = Instant::now();
    super::super::run_audio_metadata_in(&executable, &directory.path().join("first.wav")).unwrap();
    let legacy_time = legacy.elapsed();
    eprintln!("Audio covers: cold {cold_time:?}; 8 warm thumbnails {warm_time:?}; legacy single-file JSON {legacy_time:?}; worker private {:.1} MiB; peak working set {:.1} MiB", counters.PrivateUsage as f64 / 1048576.0, counters.PeakWorkingSetSize as f64 / 1048576.0);
    drop(first);
    drop(second);
    drop(absent);
    drop(fresh);
    for name in ["first.wav", "second.wav", "absent.wav"] {
        std::fs::remove_file(directory.path().join(name)).unwrap();
    }
}
