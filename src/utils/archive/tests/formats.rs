use super::*;

#[test]
fn compound_tar_suffixes_win_over_single_compressed_files() {
    for (name, format) in [
        ("a.tar", ArchiveFormat::Tar),
        ("a.tar.gz", ArchiveFormat::TarGz),
        ("a.TGZ", ArchiveFormat::TarGz),
        ("a.tar.xz", ArchiveFormat::TarXz),
        ("a.txz", ArchiveFormat::TarXz),
        ("a.tar.zst", ArchiveFormat::TarZst),
        ("a.tzst", ArchiveFormat::TarZst),
        ("a.tar.bz2", ArchiveFormat::TarBz2),
        ("a.tbz", ArchiveFormat::TarBz2),
        ("a.tar.lz4", ArchiveFormat::TarLz4),
        ("a.tar.lzma", ArchiveFormat::TarLzma),
        ("a.tlz", ArchiveFormat::TarLzma),
        ("notes.txt.gz", ArchiveFormat::Stream(Codec::Gzip)),
        ("access.log.xz", ArchiveFormat::Stream(Codec::Xz)),
        ("dump.sql.bz2", ArchiveFormat::Stream(Codec::Bzip2)),
        ("data.ZST", ArchiveFormat::Stream(Codec::Zstd)),
        ("initrd.lz4", ArchiveFormat::Stream(Codec::Lz4)),
        ("old.lzma", ArchiveFormat::Stream(Codec::Lzma)),
        ("a.zip", ArchiveFormat::Zip),
        ("a.7z", ArchiveFormat::SevenZip),
    ] {
        assert_eq!(ArchiveFormat::from_path(Path::new(name)), Some(format), "{name}");
    }
    assert_eq!(ArchiveFormat::from_path(Path::new(".gz")), None);
    assert_eq!(ArchiveFormat::from_path(Path::new("a.txt")), None);
}

#[test]
fn only_tar_codecs_with_an_encoder_are_writable() {
    for name in ["a.zip", "a.7z", "a.tar", "a.tar.zst", "a.tar.gz", "a.tar.xz", "a.tar.bz2", "a.tar.lz4"] {
        assert!(writable_format(Path::new(name)), "{name}");
        assert!(navigable_format(Path::new(name)), "{name}");
    }
    for name in ["a.tar.lzma", "notes.txt.gz", "notes.txt.zst", "a.iso", "a.rar"] {
        assert!(!writable_format(Path::new(name)), "{name}");
    }
    assert!(navigable_format(Path::new("notes.txt.gz")));
    // RAR is browsed and extracted, never written.
    assert!(navigable_format(Path::new("a.rar")));
}

#[test]
fn the_single_member_takes_the_name_without_its_suffix() {
    assert_eq!(stream_member_name(Path::new("/x/notes.txt.gz")).as_deref(), Some("notes.txt"));
    assert_eq!(stream_member_name(Path::new("/x/vmlinuz.xz")).as_deref(), Some("vmlinuz"));
}
