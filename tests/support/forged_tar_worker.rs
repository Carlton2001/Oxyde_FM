//! Standalone hostile protocol fixture; compiled by tests, never distributed.
use std::io::{self, Read, Write};
fn receive(input: &mut impl Read) -> ([u8; 16], Vec<u8>) {
    let mut header = [0; 40];
    input.read_exact(&mut header).unwrap();
    assert_eq!(&header[..4], b"OXAI");
    let meta = u32::from_le_bytes(header[32..36].try_into().unwrap()) as usize;
    let data = u32::from_le_bytes(header[36..40].try_into().unwrap()) as usize;
    assert!(meta + data <= 1024 * 1024);
    let mut body = vec![0; meta + data];
    input.read_exact(&mut body).unwrap();
    (header[8..24].try_into().unwrap(), body[..meta].to_vec())
}
fn exchange(
    input: &mut impl Read,
    output: &mut impl Write,
    session: [u8; 16],
    sequence: &mut u64,
    metadata: &str,
    data: &[u8],
) -> Vec<u8> {
    let mut header = [0; 40];
    header[..4].copy_from_slice(b"OXAI");
    header[4..6].copy_from_slice(&1u16.to_le_bytes());
    header[8..24].copy_from_slice(&session);
    header[24..32].copy_from_slice(&sequence.to_le_bytes());
    header[32..36].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
    header[36..40].copy_from_slice(&(data.len() as u32).to_le_bytes());
    output.write_all(&header).unwrap();
    output.write_all(metadata.as_bytes()).unwrap();
    output.write_all(data).unwrap();
    output.flush().unwrap();
    *sequence += 1;
    receive(input).1
}
fn numeric(metadata: &str, key: &str) -> u64 {
    let prefix = format!("\"{key}\":");
    metadata
        .split_once(&prefix)
        .unwrap()
        .1
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap()
}
fn octal(field: &mut [u8], value: u64) {
    let text = format!("{:0width$o}\0", value, width = field.len() - 1);
    field.copy_from_slice(text.as_bytes());
}
fn main() {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut gate = [0];
    input.read_exact(&mut gate).unwrap();
    assert_eq!(gate, [0xA7]);
    let (session, start) = receive(&mut input);
    let mut sequence = 0;
    let start = String::from_utf8(start).unwrap();
    if start.contains("\"Encode\"") {
        let entry = exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Source","id":0}"#,
            &[],
        );
        let entry = String::from_utf8(entry).unwrap();
        let mode = numeric(&entry, "mode");
        let modified = numeric(&entry, "seconds");
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Begin","id":0}"#,
            &[],
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Read","source":0,"offset":0,"length":4}"#,
            &[],
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"End","id":0}"#,
            &[],
        );
        let mut tar = vec![0u8; 2048];
        tar[..5].copy_from_slice(b"input");
        octal(&mut tar[100..108], mode);
        octal(&mut tar[124..136], 4);
        octal(&mut tar[136..148], modified);
        tar[156] = b'0';
        tar[257..265].copy_from_slice(b"ustar  \0");
        tar[148..156].fill(b' ');
        let checksum: u64 = tar[..512].iter().map(|byte| u64::from(*byte)).sum();
        octal(&mut tar[148..156], checksum);
        tar[512..516].copy_from_slice(b"evil");
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Write","offset":0}"#,
            &tar,
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Flush"}"#,
            &[],
        );
    } else {
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Entry","entry":{"id":0,"name":"doc","directory":false,"size":4,"mode":420,"modified":{"seconds":123,"nanos":0},"created":null,"accessed":null}}"#,
            &[],
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Begin","id":0}"#,
            &[],
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"Chunk","id":0,"offset":0}"#,
            b"evil",
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"End","id":0}"#,
            &[],
        );
        exchange(
            &mut input,
            &mut output,
            session,
            &mut sequence,
            r#"{"kind":"ManifestEnd"}"#,
            &[],
        );
    }
    exchange(
        &mut input,
        &mut output,
        session,
        &mut sequence,
        r#"{"kind":"Finished"}"#,
        &[],
    );
}
