use super::*;

#[test]
fn zip_jobs_preserve_order_content_and_requested_level() {
    let mut jobs = ZipJobs::new();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let bytes = vec![b'x'; MIB];
    for id in 0..3 {
        jobs.push(
            empty_entry(id, format!("file-{id}"), false, bytes.len() as u64),
            bytes.clone(),
            6,
        )
        .unwrap();
    }
    jobs.drain(&mut writer, &|| Ok(())).unwrap();
    let mut archive = zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
    assert_eq!(archive.len(), 3);
    for id in 0..3 {
        let mut file = archive.by_index(id).unwrap();
        assert_eq!(file.name().unwrap(), format!("file-{id}"));
        let mut decoded = Vec::new();
        file.read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, bytes);
        assert!(
            file.compressed_size() < 2000,
            "level 6 must not be replaced by level 1"
        );
    }
}

#[test]
fn admission_bounds_jobs_and_buffers_and_propagates_cancellation() {
    let mut jobs = ZipJobs::new();
    jobs.push(empty_entry(0, "a".into(), false, 0), Vec::new(), 1)
        .unwrap();
    assert!(jobs.full(1, 1));
    assert!(jobs.full(ZIP_MEMORY as u64, 16));
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let error = jobs
        .drain(&mut writer, &|| {
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancel"))
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}

#[test]
fn buffered_solid_reads_still_checkpoint() {
    struct Broker(usize);
    impl Exchange for Broker {
        fn exchange(&mut self, frame: Frame) -> io::Result<Frame> {
            assert_eq!(frame.message, Message::Checkpoint);
            self.0 += 1;
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancel"))
        }
    }
    let connection = std::sync::Arc::new(std::sync::Mutex::new(Broker(0)));
    let mut reader = Checked {
        reader: Cursor::new(vec![0; MAX_BLOCK * 2]),
        connection: connection.clone(),
        since_check: 0,
    };
    reader.read_exact(&mut vec![0; MAX_BLOCK]).unwrap();
    assert_eq!(
        reader.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(connection.lock().unwrap().0, 1);
}
