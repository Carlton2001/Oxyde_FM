use super::*;

fn small_files_tar(count: usize) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for index in 0..count {
        let content = vec![index as u8; 100];
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, format!("file{index}.txt"), content.as_slice())
            .unwrap();
    }
    tar.into_inner().unwrap()
}

#[test]
fn small_tar_members_share_source_and_progress_round_trips() {
    let tar = small_files_tar(90);
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&tar).unwrap();
    let gzip = gzip.finish().unwrap();
    let bytes = 90 * 100;
    let plain = decoded(Format::Tar, tar, None, bytes);
    let compressed = decoded(Format::TarGz, gzip, None, bytes);
    assert_eq!(plain.extracted.len(), 90);
    assert_eq!(compressed.extracted.len(), 90);
    // Entry, Begin, Chunk and End take two round trips each (checkpoint and
    // message) per member: 720 here, about 815 in all. Unbuffered source reads
    // and a progress report per read made it 1355 (plain) and 1086 (gzip).
    assert!(plain.exchanges < 900, "{}", plain.exchanges);
    assert!(compressed.exchanges < 900, "{}", compressed.exchanges);
}

#[test]
fn selecting_one_zip_member_reads_neighbour_headers_not_their_data() {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    for index in 0..20u8 {
        zip.start_file(format!("photo{index}.jpg"), options).unwrap();
        zip.write_all(&vec![index; 300 * 1024]).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    let result = decoded(Format::Zip, bytes, Some("photo7.jpg"), 300 * 1024);
    assert_eq!(result.extracted.values().next().unwrap(), &vec![7; 300 * 1024]);
    // A full window per local header made it 5.7 MB.
    assert!(result.read_bytes < 512 * 1024, "{}", result.read_bytes);
}
