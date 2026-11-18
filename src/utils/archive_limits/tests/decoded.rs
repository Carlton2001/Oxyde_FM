use super::*;

fn decode(limits: ArchiveLimits, bytes: usize) -> io::Result<u64> {
    let budget = ArchiveBudget::new(limits);
    let mut reader = BudgetReader { reader: io::repeat(0).take(bytes as u64), budget: &budget, checkpoint: &|| Ok(()) };
    io::copy(&mut reader, &mut io::sink())
}

#[test]
fn opening_a_member_may_decode_what_precedes_it_up_to_the_listing_allowance() {
    // A small member after 1 MiB of neighbours in a compressed TAR.
    let small = ArchiveLimits { max_bytes: 4, max_entries: 2, ..ArchiveLimits::OPEN_FILE };
    assert_eq!(decode(small, 1 << 20).unwrap(), 1 << 20);
    assert_eq!(ArchiveLimits::OPEN_FILE.decoded(), ArchiveLimits::LISTING.max_bytes);
    // Extraction keeps the stream bound to its output bound.
    let extraction = ArchiveLimits { max_bytes: 4, max_entries: 2, ..ArchiveLimits::EXTRACTION };
    assert!(decode(extraction, 1 << 20).is_err());
}
