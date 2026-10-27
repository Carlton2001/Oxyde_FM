//! Extended attributes a copy keeps: `user.*`, as Nautilus does, and POSIX ACLs,
//! as `cp -p` does. SELinux contexts and other namespaces belong to the destination.
use super::ExtendedAttributes;
use rustix::io::Errno;
use std::{
    ffi::OsStr,
    fs::File,
    io,
    os::unix::ffi::OsStrExt,
    path::Path,
};

fn kept(name: &[u8]) -> bool {
    name.starts_with(b"user.") || name == b"system.posix_acl_access" || name == b"system.posix_acl_default"
}

/// Reads through `read` until the buffer holds the current value, which may grow
/// between the size query and the read.
fn sized(mut read: impl FnMut(&mut [u8]) -> rustix::io::Result<usize>) -> rustix::io::Result<Vec<u8>> {
    loop {
        let size = read(&mut [])?;
        let mut buffer = vec![0_u8; size];
        match read(&mut buffer) {
            Ok(length) => {
                buffer.truncate(length);
                return Ok(buffer);
            }
            Err(Errno::RANGE) => continue,
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn read(file: &File) -> io::Result<ExtendedAttributes> {
    let names = match sized(|buffer| rustix::fs::flistxattr(file, buffer)) {
        Ok(names) => names,
        // A file system without extended attributes has nothing to keep.
        Err(Errno::OPNOTSUPP) => return Ok(ExtendedAttributes::default()),
        Err(error) => return Err(error.into()),
    };
    let mut attributes = Vec::new();
    for name in names.split(|byte| *byte == 0).filter(|name| kept(name)) {
        let name = OsStr::from_bytes(name);
        match sized(|buffer| rustix::fs::fgetxattr(file, name, buffer)) {
            Ok(value) => attributes.push((name.to_os_string(), value)),
            // Removed since the listing.
            Err(Errno::NODATA) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(ExtendedAttributes(attributes))
}

/// Writes each attribute before the mode is applied: writing `user.*` needs write
/// permission, and a later mode change keeps the ACL in step with the mode. An
/// attribute the destination refuses does not stop the others: the error names
/// every refused one, for the caller to report without failing the copy.
pub(super) fn write(file: &File, attributes: &ExtendedAttributes) -> io::Result<()> {
    let mut refused = Vec::new();
    for (name, value) in &attributes.0 {
        if let Err(error) = rustix::fs::fsetxattr(file, name.as_os_str(), value, rustix::fs::XattrFlags::empty()) {
            refused.push(format!("{}: {}", name.to_string_lossy(), io::Error::from(error)));
        }
    }
    if refused.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(format!("extended attributes not kept ({})", refused.join("; "))))
    }
}

/// Copies them from one open file to another; a failure is logged, never fatal.
pub(super) fn copy(input: &File, output: &File, source: &Path) {
    if let Err(error) = read(input).and_then(|attributes| write(output, &attributes)) {
        log::warn!("Copy of {}: {error}", source.display());
    }
}

#[cfg(test)]
#[path = "linux_extended_attributes/tests/attributes.rs"]
mod tests;
