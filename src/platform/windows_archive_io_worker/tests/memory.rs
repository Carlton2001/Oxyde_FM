use super::*;
use crate::utils::archive_io_protocol::{Format, Limits, Quality};

#[test]
fn only_seven_encoding_uses_the_larger_envelope_and_navigation_stays_reserved() {
    let limits = Limits {
        entries: 10,
        bytes: 1024,
        depth: 8,
        seconds: 30,
    };
    for format in [Format::Zip, Format::SevenZip, Format::Tar, Format::TarZst] {
        let operation = Operation::Encode {
            format,
            quality: Quality::Normal,
            entries: 1,
            output_limit: 2048,
            limits,
        };
        let expected = if format == Format::SevenZip {
            2 * MEMORY_BYTES
        } else {
            MEMORY_BYTES
        };
        assert_eq!(worker_memory(&operation), expected);
        assert!(
            worker_memory(&operation) + 512 * 1024 * 1024
                <= super::super::windows_worker_budget::MEMORY_BYTES
        );
        let decode = Operation::Decode {
            format,
            source_size: 1024,
            selected: None,
            limits,
        };
        assert_eq!(worker_memory(&decode), MEMORY_BYTES);
    }
}
