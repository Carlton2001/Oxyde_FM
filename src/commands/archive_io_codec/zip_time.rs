//! ZIP member dates. The header holds a DOS date in local time, to the even
//! second, from 1980 to 2107; Windows tools add an NTFS field and Unix ones an
//! extended timestamp, both in UTC, which take precedence when present.
use crate::utils::archive_io_protocol::Timestamp;
use chrono::{Local, TimeZone, Utc};
use std::io::Read;

/// Seconds from 1601 (Windows file times) to 1970.
const FILE_TIME_TO_UNIX: i64 = 11_644_473_600;

/// The DOS date written for `time`, when it has one in that range.
pub(crate) fn to_zip(time: Option<Timestamp>) -> Option<zip::DateTime> {
    let time = time?;
    let utc = Utc.timestamp_opt(time.seconds, time.nanos).single()?;
    zip::DateTime::try_from(utc.with_timezone(&Local).naive_local()).ok()
}

/// The member's modification date, or `None` when it records none.
pub(crate) fn from_zip<R: Read>(file: &zip::read::ZipFile<'_, R>) -> Option<Timestamp> {
    let utc = file.extra_data_fields().find_map(|field| match field {
        zip::ExtraField::Ntfs(ntfs) if ntfs.mtime() != 0 => {
            let ticks = ntfs.mtime();
            Some(Timestamp {
                seconds: (ticks / 10_000_000) as i64 - FILE_TIME_TO_UNIX,
                nanos: (ticks % 10_000_000) as u32 * 100,
            })
        }
        zip::ExtraField::ExtendedTimestamp(extended) => extended
            .mod_time()
            .map(|seconds| Timestamp { seconds: i64::from(seconds), nanos: 0 }),
        _ => None,
    });
    utc.or_else(|| {
        let naive = chrono::NaiveDateTime::try_from(file.last_modified()?).ok()?;
        let time = Local.from_local_datetime(&naive).earliest()?;
        Some(Timestamp { seconds: time.timestamp(), nanos: 0 })
    })
}

#[cfg(test)]
#[path = "tests/zip_time.rs"]
mod tests;
