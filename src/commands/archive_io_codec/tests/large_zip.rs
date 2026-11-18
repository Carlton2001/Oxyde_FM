use super::*;
use std::io::SeekFrom;
use std::sync::{Arc, Mutex};

/// Incompressible input whose DEFLATE output passes 4 GiB.
const SIZE: u64 = 4200 * 1024 * 1024;

/// Pseudo-random bytes computed from their offset, so any block can be served.
fn byte_block(offset: u64, length: usize) -> Vec<u8> {
    (0..length as u64).map(|i| {
        let mut z = (offset + i) / 8 + 0x9E37_79B9_7F4A_7C15;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)).to_le_bytes()[((offset + i) % 8) as usize]
    }).collect()
}

/// Serves the generated source and writes the archive to a file, not memory.
struct FileBroker { output: std::fs::File, entry: Entry }
impl Exchange for FileBroker {
    fn exchange(&mut self, frame: Frame) -> io::Result<Frame> {
        match &frame.message {
            Message::Read { source, offset, length } => {
                return Ok(Frame::data(Message::Bytes { source: *source, offset: *offset }, byte_block(*offset, *length as usize)));
            }
            Message::Write { offset } => {
                self.output.seek(SeekFrom::Start(*offset))?;
                self.output.write_all(&frame.data)?;
            }
            Message::Source { .. } => return Ok(Frame::new(Message::Entry { entry: self.entry.clone() })),
            _ => {}
        }
        Ok(Frame::new(Message::Ack { pause: false, turbo: false }))
    }
}

/// About a minute and 4.4 GB of disk: run with `--ignored`. zlib-rs counters
/// are 32 bits on Windows; with overflow checks they panicked the worker.
#[test]
#[ignore]
fn a_zip_member_compressing_past_4_gib_is_written() {
    let directory = tempfile::tempdir().unwrap();
    let dir = directory.path();
    let broker = FileBroker {
        output: std::fs::File::options().read(true).write(true).create(true).truncate(true).open(dir.join("out.zip")).unwrap(),
        entry: empty_entry(0, "big.bin".into(), false, SIZE),
    };
    let result = run(Arc::new(Mutex::new(broker)), Operation::Encode {
        format: Format::Zip, quality: Quality::Fast, entries: 1, output_limit: u64::MAX / 2, password: None,
        limits: Limits { entries: 10, bytes: SIZE * 2, depth: 64, seconds: 3600, decoded: SIZE * 2 },
    });
    result.unwrap();
    assert!(std::fs::metadata(dir.join("out.zip")).unwrap().len() > 1 << 32);
}
