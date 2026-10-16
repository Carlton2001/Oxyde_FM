//! Display-only enumeration: Shell identities plus a single sidecar read.
use std::path::PathBuf;
use crate::models::TrashEntry;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_RecycleBinFolder, IEnumShellItems, IShellItem,
    SHGetKnownFolderItem, KF_FLAG_DEFAULT, SIGDN_DESKTOPABSOLUTEPARSING,
};

pub(crate) fn list() -> Result<Vec<TrashEntry>, String> {
    let mut entries = Vec::new();
    stream(&mut |batch| { entries.extend(batch); true })?;
    Ok(entries)
}

pub(crate) fn stream(emit: &mut dyn FnMut(Vec<TrashEntry>) -> bool) -> Result<(), String> {
    let _apartment = super::super::windows_com::ComApartment::sta_or_existing()
        .map_err(|error| error.to_string())?;
    let started = std::time::Instant::now();
    let mut enrichment = std::time::Duration::ZERO;
    let mut entries = Vec::new();
    let mut count = 0;
    let mut first = true;
    let mut published = started;
    unsafe {
        let folder: IShellItem = SHGetKnownFolderItem(
            &FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None,
        ).map_err(|error| error.to_string())?;
        let enumerator: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems)
            .map_err(|error| error.to_string())?;
        loop {
            let mut batch: [Option<IShellItem>; 64] = std::array::from_fn(|_| None);
            let mut fetched = 0;
            enumerator.Next(&mut batch, Some(&mut fetched))
                .map_err(|error| error.to_string())?;
            if fetched == 0 { break; }
            for item in batch.into_iter().take(fetched as usize) {
                let item = item.ok_or("Missing recycle bin Shell item")?;
                let path = super::super::windows_com::take_string(
                    item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)
                        .map_err(|error| error.to_string())?,
                ).map_err(|error| error.to_string())?;
                let before = std::time::Instant::now();
                entries.push(read_entry(PathBuf::from(path))?);
                enrichment += before.elapsed();
                count += 1;
                if (first && entries.len() >= 64) || published.elapsed().as_millis() >= 100 {
                    if !emit(std::mem::take(&mut entries)) { return Ok(()); }
                    first = false;
                    published = std::time::Instant::now();
                }
            }
        }
    }
    log::info!(target: "oxyde_lib::trash_listing",
        "mode=native_display items={} shell_s={:.6} enrichment_s={:.6}",
        count, started.elapsed().saturating_sub(enrichment).as_secs_f64(),
        enrichment.as_secs_f64());
    if !entries.is_empty() || first { emit(entries); }
    Ok(())
}

fn read_entry(path: PathBuf) -> Result<TrashEntry, String> {
    let name = path.file_name().ok_or("Missing recycle bin filename")?.to_string_lossy();
    if !name.get(..2).is_some_and(|prefix| prefix.eq_ignore_ascii_case("$R")) {
        return Err("Unexpected recycle bin payload name".into());
    }
    let sidecar = path.with_file_name(format!("$I{}", &name[2..]));
    let mut file = super::super::platform().open_archive_source(&sidecar, false)
        .map_err(|error| error.to_string())?;
    let bytes = super::metadata_bytes(&mut file).map_err(|error| error.to_string())?;
    let original = super::parse_original_path(&bytes).ok_or("Invalid recycle bin metadata")?;
    let deleted_time = deletion_time(&bytes).ok_or("Invalid recycle bin deletion time")?;
    let metadata = std::fs::metadata(&path).ok();
    let is_dir = metadata.as_ref().is_some_and(|metadata| metadata.is_dir());
    let size = if is_dir { 0 } else { metadata.as_ref().map_or(0, |metadata| metadata.len()) };
    let modified = metadata.as_ref().and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(deleted_time, |duration| duration.as_millis() as u64);
    Ok(TrashEntry {
        name: original.file_name().unwrap_or(original.as_os_str()).to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        original_path: original.to_string_lossy().into_owned(),
        is_dir, size, deleted_time, modified,
    })
}

fn deletion_time(bytes: &[u8]) -> Option<u64> {
    // FILETIME uses 100 ns ticks since 1601; the public listing uses Unix ms.
    let ticks = u64::from_le_bytes(bytes.get(16..24)?.try_into().ok()?);
    Some(ticks.saturating_sub(116_444_736_000_000_000) / 10_000)
}

#[cfg(test)]
#[path = "listing/tests/listing.rs"]
mod tests;
