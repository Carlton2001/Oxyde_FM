use super::*;

fn sample() -> Vec<u8> {
    (0..300_000u32).map(|value| (value * 7 % 253) as u8).collect()
}

fn decode(codec: Codec, bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    codec.decoder(bytes)?.read_to_end(&mut output)?;
    Ok(output)
}

fn encode(codec: Codec, content: &[u8], level: Level, threads: usize) -> Vec<u8> {
    let mut encoder = codec.encoder(Vec::new(), level, threads).unwrap();
    encoder.write_all(content).unwrap();
    encoder.flush().unwrap();
    encoder.finish().unwrap()
}

const WRITABLE: [Codec; 5] = [Codec::Gzip, Codec::Xz, Codec::Bzip2, Codec::Zstd, Codec::Lz4];

#[test]
fn every_writable_codec_roundtrips_at_every_level_and_thread_count() {
    let content = sample();
    for codec in WRITABLE {
        assert!(codec.writable());
        for level in [Level::Fast, Level::Normal, Level::Best] {
            for threads in [1, 4] {
                let bytes = encode(codec, &content, level, threads);
                assert!(bytes.len() < content.len(), "{codec:?} {level:?}");
                assert_eq!(decode(codec, &bytes).unwrap(), content, "{codec:?} {level:?} {threads}");
            }
        }
    }
    assert!(!Codec::Lzma.writable());
    assert!(Codec::Lzma.encoder(Vec::new(), Level::Fast, 1).is_err());
}

#[test]
fn concatenated_members_decode_as_one_stream() {
    for codec in WRITABLE {
        let mut bytes = encode(codec, b"first ", Level::Fast, 1);
        bytes.extend(encode(codec, b"second", Level::Fast, 1));
        assert_eq!(decode(codec, &bytes).unwrap(), b"first second", "{codec:?}");
    }
}

#[test]
fn legacy_lzma_streams_decode() {
    let options = xz2::stream::LzmaOptions::new_preset(1).unwrap();
    let mut encoder = xz2::write::XzEncoder::new_stream(
        Vec::new(),
        xz2::stream::Stream::new_lzma_encoder(&options).unwrap(),
    );
    encoder.write_all(&sample()).unwrap();
    assert_eq!(decode(Codec::Lzma, &encoder.finish().unwrap()).unwrap(), sample());
}

#[test]
fn empty_foreign_and_truncated_inputs_are_refused() {
    let content = sample();
    for codec in WRITABLE {
        assert!(decode(codec, b"").is_err(), "{codec:?} accepted an empty file");
        assert!(decode(codec, b"plain text").is_err(), "{codec:?} accepted text");
        let bytes = encode(codec, &content, Level::Fast, 1);
        assert!(decode(codec, &bytes[..bytes.len() / 2]).is_err(), "{codec:?} accepted a truncation");
    }
    assert!(decode(Codec::Lzma, b"").is_err());
    assert!(decode(Codec::Lzma, &[0xff; 32]).is_err());
}

#[test]
fn single_file_suffixes_name_their_codec() {
    for (extension, codec) in [
        ("gz", Codec::Gzip),
        ("xz", Codec::Xz),
        ("bz2", Codec::Bzip2),
        ("zst", Codec::Zstd),
        ("lz4", Codec::Lz4),
        ("lzma", Codec::Lzma),
    ] {
        assert_eq!(Codec::from_extension(extension), Some(codec));
    }
    assert_eq!(Codec::from_extension("zip"), None);
}
