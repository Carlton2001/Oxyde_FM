//! Shell New labels declared by packages registered for the current user.

use super::{display_string, registered_extension};
use roxmltree::Document;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use windows::Management::Deployment::{PackageManager, PackageTypes};
use windows::core::HSTRING;

const UAP: &str = "http://schemas.microsoft.com/appx/manifest/uap/windows10";
const UAP4: &str = "http://schemas.microsoft.com/appx/manifest/uap/windows10/4";
const UAP10: &str = "http://schemas.microsoft.com/appx/manifest/uap/windows10/10";
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;

/// Visits each label and extension, and whether creating the file launches the package.
pub(super) fn visit_installed(
    mut visit: impl FnMut(&str, &str, bool),
) -> windows::core::Result<()> {
    let _com = super::super::windows_com::ComApartment::mta_or_existing()?;
    let packages = PackageManager::new()?.FindPackagesByUserSecurityIdWithPackageTypes(
        &HSTRING::new(),
        PackageTypes::Main | PackageTypes::Optional,
    )?;
    for package in packages {
        let (Ok(id), Ok(path)) = (package.Id(), package.InstalledPath()) else {
            continue;
        };
        let (Ok(name), Ok(full_name)) = (id.Name(), id.FullName()) else {
            continue;
        };
        let path = PathBuf::from(path.to_os_string());
        let Ok(file) = File::open(path.join("AppxManifest.xml")) else {
            continue;
        };
        let mut manifest = String::new();
        if file
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_string(&mut manifest)
            .is_err()
            || manifest.len() as u64 > MAX_MANIFEST_BYTES
        {
            continue;
        }
        let name = name.to_string_lossy();
        let full_name = full_name.to_string_lossy();
        visit_manifest(&manifest, |label, extension, program| {
            if let Some(label) = package_display_name(&name, &full_name, label) {
                visit(&label, extension, program);
            }
        });
    }
    Ok(())
}

fn visit_manifest(manifest: &str, mut visit: impl FnMut(&str, &str, bool)) {
    let Ok(document) = Document::parse(manifest.trim_start_matches('\u{feff}')) else {
        return;
    };
    for node in document
        .descendants()
        .filter(|node| node.has_tag_name((UAP, "FileType")))
    {
        let Some(list) = node
            .parent()
            .filter(|parent| parent.has_tag_name((UAP, "SupportedFileTypes")))
        else {
            continue;
        };
        let Some(association) = list
            .parent()
            .filter(|parent| parent.has_tag_name((UAP, "FileTypeAssociation")))
        else {
            continue;
        };
        if !node.has_attribute((UAP4, "ShellNewDisplayName"))
            && !node.has_attribute((UAP4, "ShellNewFileName"))
            && !node.has_attribute((UAP10, "ShellNewCommandParameters"))
        {
            continue;
        }
        let Some(extension) = node
            .text()
            .map(str::trim)
            .filter(|name| registered_extension(name))
        else {
            continue;
        };
        let label = node.attribute((UAP4, "ShellNewDisplayName")).or_else(|| {
            association
                .children()
                .find(|child| child.has_tag_name((UAP, "DisplayName")))?
                .text()
        });
        let program = node.has_attribute((UAP10, "ShellNewCommandParameters"));
        if let Some(label) = label.map(str::trim).filter(|label| !label.is_empty()) {
            visit(label, extension, program);
        }
    }
}

fn package_display_name(name: &str, full_name: &str, label: &str) -> Option<String> {
    let Some(resource) = label.strip_prefix("ms-resource:") else {
        return Some(label.to_owned());
    };
    let resource = if let Some(path) = resource.strip_prefix("//") {
        if path.starts_with('/') {
            format!("ms-resource://{name}{path}")
        } else {
            label.to_owned()
        }
    } else {
        let path = resource.trim_start_matches('/');
        if path.contains('/') {
            format!("ms-resource://{name}/{path}")
        } else {
            format!("ms-resource://{name}/Resources/{path}")
        }
    };
    display_string(format!("@{{{full_name}?{resource}}}"))
}

#[cfg(test)]
#[path = "tests/package_types.rs"]
mod tests;
