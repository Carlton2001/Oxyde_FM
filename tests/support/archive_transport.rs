use super::*;

#[test]
fn partial_frame_cannot_block_supervisor_deadline_or_cancellation() {
    native_probe::executable();
    for cancel in [false, true] {
        for partial_header in [true, false] {
            let mut wire = utils::archive_io_protocol::Wire::new([7; 16]);
            let mut bytes = Vec::new();
            wire.write(&mut bytes, &Frame::new(Message::Source { id: 0 }))
                .unwrap();
            let mut partial = Vec::new();
            wire.write(&mut partial, &Frame::new(Message::Checkpoint))
                .unwrap();
            let end = utils::archive_io_protocol::HEADER - usize::from(partial_header);
            bytes.extend_from_slice(&partial[..end]);
            let mut command = native_probe::command("frames");
            command.arg(
                bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            );
            let ready = std::cell::Cell::new(None::<Instant>);
            let started = Instant::now();
            let error = windows_archive_io_worker::supervise(
                command,
                [7; 16],
                encode_operation(Format::Zip, Quality::Fast),
                &|| {
                    Ok(Control {
                        cancelled: cancel
                            && ready
                                .get()
                                .is_some_and(|at| at.elapsed() >= Duration::from_millis(250)),
                        ..Control::default()
                    })
                },
                &mut |frame| {
                    assert_eq!(frame.message, Message::Source { id: 0 });
                    ready.set(Some(Instant::now()));
                    noop(frame)
                },
                if cancel {
                    Duration::from_secs(10)
                } else {
                    Duration::from_millis(500)
                },
                1024 * 1024 * 1024,
            )
            .err()
            .unwrap();
            assert!(
                ready.get().is_some(),
                "partial-frame child did not reach readiness"
            );
            assert_eq!(
                error.kind(),
                if cancel {
                    io::ErrorKind::Interrupted
                } else {
                    io::ErrorKind::TimedOut
                }
            );
            assert!(started.elapsed() < Duration::from_secs(4));
        }
    }
    // A fresh real session still acquires admission and completes afterwards.
    encoded(Format::Zip, Quality::Fast);
}
