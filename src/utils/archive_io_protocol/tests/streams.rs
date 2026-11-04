use super::*;
use crate::utils::archive_codec::Codec;

fn start(format: Format, stream_name: Option<&str>) -> io::Result<()> {
    Frame::new(Message::Start {
        operation: Operation::Decode {
            format,
            source_size: 1,
            selected: None,
            stream_name: stream_name.map(str::to_owned),
            password: None,
            limits: Limits { entries: 1, bytes: 1, depth: 1, seconds: 1 },
        },
    })
    .validate()
}

#[test]
fn only_a_single_compressed_file_carries_one_safe_member_name() {
    assert!(start(Format::Stream(Codec::Gzip), Some("notes.txt")).is_ok());
    for name in [None, Some(""), Some("../notes"), Some("dir/notes"), Some("dir\\notes")] {
        assert!(start(Format::Stream(Codec::Gzip), name).is_err(), "{name:?}");
    }
    assert!(start(Format::TarGz, Some("notes.txt")).is_err());
    assert!(start(Format::TarGz, None).is_ok());
}

#[test]
fn encoders_exist_for_writable_tar_codecs_only() {
    let encode = |format| {
        Frame::new(Message::Start {
            operation: Operation::Encode {
                format,
                quality: Quality::Fast,
                entries: 1,
                output_limit: 1,
                password: None,
                limits: Limits { entries: 1, bytes: 1, depth: 1, seconds: 1 },
            },
        })
        .validate()
    };
    for format in [Format::Zip, Format::SevenZip, Format::Tar, Format::TarZst, Format::TarGz, Format::TarXz, Format::TarBz2, Format::TarLz4] {
        assert!(encode(format).is_ok(), "{format:?}");
    }
    for format in [Format::TarLzma, Format::Stream(Codec::Gzip), Format::Iso] {
        assert!(encode(format).is_err(), "{format:?}");
    }
}
