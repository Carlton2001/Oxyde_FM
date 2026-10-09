//! Literal paths only: SHFileOperation interprets wildcards and NUL separators.
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, PathBuf, Prefix};

/// Explicit selections must never acquire unselected Shell-connected items.
pub(super) fn operation_flags(recycle: bool) -> u16 {
    use windows::Win32::UI::Shell::{
        FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_NO_CONNECTED_ELEMENTS, FOF_NO_UI,
        FOF_SILENT,
    };
    let mut flags =
        FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0 | FOF_NO_CONNECTED_ELEMENTS.0;
    if recycle {
        flags |= FOF_ALLOWUNDO.0;
    } else {
        flags |= FOF_NO_UI.0;
    }
    flags as u16
}

pub(super) fn source_buffer(paths: &[PathBuf]) -> Result<Vec<u16>, String> {
    let mut buffer = Vec::new();
    for path in paths {
        let fail = || "Invalid literal path for Windows deletion".to_string();
        if !path.is_absolute() || path.file_name().is_none() {
            return Err(fail());
        }
        let Some(Component::Prefix(prefix)) = path.components().next() else {
            return Err(fail());
        };
        let extended = match prefix.kind() {
            Prefix::Disk(_) | Prefix::UNC(_, _) => false,
            Prefix::VerbatimDisk(_) | Prefix::VerbatimUNC(_, _) => true,
            _ => return Err(fail()),
        };
        let mut units: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .map(|unit| {
                if unit == b'/' as u16 {
                    b'\\' as u16
                } else {
                    unit
                }
            })
            .collect();
        if units.iter().any(|unit| [0, b'*' as u16].contains(unit)) {
            return Err(fail());
        }
        if extended {
            if matches!(prefix.kind(), Prefix::VerbatimUNC(_, _)) {
                units.splice(..8, [b'\\' as u16, b'\\' as u16]);
            } else {
                units.drain(..4);
            }
        }
        if units.contains(&(b'?' as u16)) {
            return Err(fail());
        }
        // Reject aliases and streams rather than letting the legacy Shell choose
        // an object other than the literal entry selected by the caller.
        let rest = if units.get(1) == Some(&(b':' as u16)) {
            &units[3..]
        } else {
            &units[..]
        };
        for part in rest
            .split(|unit| *unit == b'\\' as u16)
            .filter(|part| !part.is_empty())
        {
            if part.contains(&(b':' as u16)) || matches!(part.last(), Some(32 | 46)) {
                return Err(fail());
            }
        }
        buffer.extend(units);
        buffer.push(0);
    }
    buffer.push(0);
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_selection_never_packs_its_companion_and_both_modes_disable_connected_items() {
        use windows::Win32::UI::Shell::{FOF_ALLOWUNDO, FOF_NO_CONNECTED_ELEMENTS};
        let source = PathBuf::from(r"C:\fixtures\Page.html");
        let buffer = source_buffer(&[source]).unwrap();
        assert_eq!(
            String::from_utf16(&buffer).unwrap(),
            "C:\\fixtures\\Page.html\0\0"
        );
        for recycle in [false, true] {
            let flags = u32::from(operation_flags(recycle));
            assert_ne!(flags & FOF_NO_CONNECTED_ELEMENTS.0, 0);
            assert_eq!(flags & FOF_ALLOWUNDO.0 != 0, recycle);
        }
    }

    #[test]
    fn refuses_ambiguous_sources_before_any_shell_call() {
        for path in [
            "relative.txt",
            r"C:relative.txt",
            r"C:\",
            r"C:\a*",
            r"C:\a?",
            "C:\\safe\0C:\\other",
            r"\\.\C:\file",
            r"C:\file:stream",
            r"C:\alias.\file",
            r"C:\alias \file",
            r"C:\a\..\file",
            r"C:\a\file.",
        ] {
            assert!(
                source_buffer(&[PathBuf::from(r"C:\safe"), PathBuf::from(path)]).is_err(),
                "{path:?}"
            );
        }
    }

    #[test]
    fn preserves_native_unicode_and_packs_exactly_two_terminators() {
        let paths = [
            PathBuf::from("C:/école.txt"),
            PathBuf::from(r"\\?\UNC\server\share\file"),
        ];
        let buffer = source_buffer(&paths).unwrap();
        assert_eq!(
            String::from_utf16(&buffer).unwrap(),
            "C:\\école.txt\0\\\\server\\share\\file\0\0"
        );
        assert_eq!(
            source_buffer(&[PathBuf::from(r"\\?\C:\file")]).unwrap(),
            "C:\\file\0\0".encode_utf16().collect::<Vec<_>>()
        );
    }
}
