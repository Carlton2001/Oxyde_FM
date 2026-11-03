use super::*;
use std::io::Write;

fn compressed(codec: crate::utils::archive_codec::Codec, content: &[u8]) -> Vec<u8> {
    use crate::utils::archive_codec::Level;
    let mut encoder = codec.encoder(Vec::new(), Level::Fast, 1).unwrap();
    encoder.write_all(content).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn a_single_compressed_file_lists_its_one_member_with_the_decoded_size() {
    use crate::utils::archive_codec::Codec;
    let temp = tempfile::tempdir().unwrap();
    let content = vec![b'x'; 100_000];
    for (extension, codec) in [("gz", Codec::Gzip), ("xz", Codec::Xz), ("bz2", Codec::Bzip2), ("zst", Codec::Zstd), ("lz4", Codec::Lz4)] {
        let path = temp.path().join(format!("journal.log.{extension}"));
        std::fs::write(&path, compressed(codec, &content)).unwrap();
        let index = indexed_files(&path, &|| Ok(())).unwrap();
        assert_eq!(index.len(), 1, "{extension}");
        assert_eq!(index[0].name, "journal.log");
        assert_eq!(Path::new(&index[0].path), path.join("journal.log"));
        assert_eq!(index[0].size, content.len() as u64);
        assert!(!index[0].is_dir);
        let listing = super::super::archive::list_archive_contents(path.to_string_lossy().into_owned(), String::new()).unwrap();
        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].name, "journal.log");
        let limits = ArchiveLimits { max_bytes: 1_000, max_entries: 1, ..ArchiveLimits::LISTING };
        assert!(indexed_with_limits(&path, limits, &|| Ok(())).is_err(), "{extension} listing is unbounded");
        std::fs::write(&path, b"not compressed").unwrap();
        assert!(indexed_files(&path, &|| Ok(())).is_err(), "{extension} listed foreign content");
    }
}

#[test]
fn lz4_and_lzma_tars_list_their_members() {
    use crate::utils::archive_codec::Codec;
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(4);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, "docs/note.txt", &b"safe"[..]).unwrap();
    let tar = builder.into_inner().unwrap();
    let options = xz2::stream::LzmaOptions::new_preset(1).unwrap();
    let mut lzma = xz2::write::XzEncoder::new_stream(Vec::new(), xz2::stream::Stream::new_lzma_encoder(&options).unwrap());
    lzma.write_all(&tar).unwrap();
    let temp = tempfile::tempdir().unwrap();
    for (name, bytes) in [("a.tar.lz4", compressed(Codec::Lz4, &tar)), ("a.tar.lzma", lzma.finish().unwrap())] {
        let path = temp.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let index = indexed_files(&path, &|| Ok(())).unwrap();
        assert_eq!(Path::new(&index[0].path), path.join("docs/note.txt"), "{name}");
    }
}
