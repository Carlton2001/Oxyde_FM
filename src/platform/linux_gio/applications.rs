//! Applications for « Open With », from GIO's desktop entries and `mimeapps.list`, as
//! Nautilus's application chooser reads and writes them.
use super::library::{gio, GList, Gio, Object};
use super::{borrowed, failure, taken, text, Owned};
use crate::models::{ApplicationChoice, ApplicationFit};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The file types of `paths`, without repeats, in their order.
fn content_types(paths: &[String]) -> Vec<String> {
    let mut types: Vec<String> = Vec::new();
    for path in paths {
        let path = Path::new(path);
        let name = crate::platform::linux_mime::mime_type(path, path.is_dir(), "").name;
        if !types.contains(&name) {
            types.push(name);
        }
    }
    types
}

/// Takes a list of applications, each with the reference it holds.
unsafe fn applications(gio: &'static Gio, list: *mut GList) -> Vec<Owned> {
    let mut found = Vec::new();
    let mut cell = list;
    while !cell.is_null() {
        // SAFETY: a list GIO returned, each cell holding one reference now owned here.
        unsafe {
            found.extend(Owned::new(gio, (*cell).data));
            cell = (*cell).next;
        }
    }
    // SAFETY: the cells alone; their references were taken above.
    unsafe { (gio.g_list_free)(list) };
    found
}

fn id_of(gio: &Gio, application: Object) -> String {
    // SAFETY: a live application; the identifier belongs to it.
    unsafe { borrowed((gio.g_app_info_get_id)(application)) }
}

/// Identifiers of the applications GIO lists for each type, kept when every type lists them.
fn common(gio: &'static Gio, types: &[String], list: unsafe extern "C" fn(*const std::ffi::c_char) -> *mut GList) -> Vec<(String, Owned)> {
    let mut shared: Option<Vec<(String, Owned)>> = None;
    for content_type in types {
        let name = text(content_type);
        // SAFETY: a type name; the list is taken whole.
        let found = unsafe { applications(gio, list(name.as_ptr())) };
        let found: Vec<_> = found.into_iter().map(|app| (id_of(gio, app.1), app)).filter(|(id, _)| !id.is_empty()).collect();
        shared = Some(match shared {
            None => found,
            Some(kept) => {
                let ids: HashSet<_> = found.iter().map(|(id, _)| id.clone()).collect();
                kept.into_iter().filter(|(id, _)| ids.contains(id)).collect()
            }
        });
    }
    shared.unwrap_or_default()
}

/// The application opening every type by default, when they share one.
fn shared_default(gio: &'static Gio, types: &[String]) -> Option<String> {
    let mut default = None;
    for content_type in types {
        let name = text(content_type);
        // SAFETY: a type name; the application returned is owned.
        let app = Owned::new(gio, unsafe { (gio.g_app_info_get_default_for_type)(name.as_ptr(), 0) })?;
        let id = id_of(gio, app.1);
        if default.get_or_insert_with(|| id.clone()) != &id {
            return None;
        }
    }
    default
}

fn shown(gio: &Gio, application: Object) -> bool {
    // SAFETY: a live application.
    unsafe { (gio.g_app_info_should_show)(application) != 0 }
}

/// Recommended, then related, then every other application, as GNOME sorts them.
pub(in crate::platform) fn choices(paths: &[String]) -> Result<Vec<ApplicationChoice>, String> {
    let gio = gio().ok_or("GIO is unavailable")?;
    let types = content_types(paths);
    if types.is_empty() {
        return Ok(Vec::new());
    }
    let default = shared_default(gio, &types);
    let recommended = common(gio, &types, gio.g_app_info_get_recommended_for_type);
    let all = common(gio, &types, gio.g_app_info_get_all_for_type);
    // SAFETY: the list is taken whole.
    let every = unsafe { applications(gio, (gio.g_app_info_get_all)()) };
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    let groups = [
        (ApplicationFit::Recommended, recommended),
        (ApplicationFit::Related, all),
        (ApplicationFit::Other, every.into_iter().map(|app| (id_of(gio, app.1), app)).collect()),
    ];
    for (fit, group) in groups {
        let start = found.len();
        for (id, app) in group {
            // The default stays listed, even when its entry hides it from menus.
            let is_default = default.as_ref() == Some(&id);
            if id.is_empty() || !(is_default || shown(gio, app.1)) || !seen.insert(id.clone()) {
                continue;
            }
            found.push(choice(gio, id, &app, is_default, fit));
        }
        if fit != ApplicationFit::Recommended {
            found[start..].sort_by_cached_key(|choice: &ApplicationChoice| choice.name.to_lowercase());
        }
    }
    // The default leads, as the application opening the files now.
    if let Some(index) = found.iter().position(|choice| choice.is_default) {
        let default = found.remove(index);
        found.insert(0, ApplicationChoice { fit: ApplicationFit::Recommended, ..default });
    }
    Ok(found)
}

fn choice(gio: &Gio, id: String, app: &Owned, is_default: bool, fit: ApplicationFit) -> ApplicationChoice {
    // SAFETY: a live application; its name and icon belong to it, the icon text is copied.
    let (name, icon) = unsafe {
        let name = borrowed((gio.g_app_info_get_display_name)(app.1));
        let icon = (gio.g_app_info_get_icon)(app.1);
        let icon = if icon.is_null() { None } else { taken(gio, (gio.g_icon_to_string)(icon)) };
        (name, icon)
    };
    let name = if name.is_empty() { id.trim_end_matches(".desktop").to_owned() } else { name };
    let icon = icon.and_then(|icon| icon_file(&String::from_utf8_lossy(&icon)));
    ApplicationChoice { id, name, icon, is_default, fit }
}

/// The image of a themed icon, from `hicolor` where applications install theirs.
fn icon_file(icon: &str) -> Option<PathBuf> {
    if icon.starts_with('/') {
        return Some(PathBuf::from(icon)).filter(|path| path.is_file());
    }
    // A themed icon of several names serializes as `. GThemedIcon name…`.
    let names: Vec<&str> = match icon.strip_prefix(". GThemedIcon ") {
        Some(names) => names.split_whitespace().collect(),
        None if icon.starts_with(". ") => return None,
        None => vec![icon],
    };
    let mut bases = vec![dirs::data_dir().unwrap_or_default()];
    let data_dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    bases.extend(std::env::split_paths(&data_dirs));
    let home_icons = dirs::home_dir().map(|home| home.join(".icons"));
    const SIZES: [&str; 10] = ["scalable", "512x512", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24", "16x16"];
    for name in names.iter().filter(|name| !name.contains('/')) {
        let themes = home_icons.iter().cloned().chain(bases.iter().map(|base| base.join("icons")));
        for theme in themes {
            for size in SIZES {
                for extension in ["svg", "png"] {
                    let path = theme.join("hicolor").join(size).join("apps").join(format!("{name}.{extension}"));
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
        for base in &bases {
            for extension in ["svg", "png"] {
                let path = base.join("pixmaps").join(format!("{name}.{extension}"));
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Opens `paths` with the application `id`. `remember` makes it the default for their
/// types, as « Always use for this file type »; otherwise it is recorded as last used,
/// which GIO then recommends.
pub(in crate::platform) fn launch(id: &str, paths: &[String], remember: bool) -> Result<(), String> {
    let gio = gio().ok_or("GIO is unavailable")?;
    let name = text(id);
    // SAFETY: an identifier; the application returned is owned.
    let app = Owned::new(gio, unsafe { (gio.g_desktop_app_info_new)(name.as_ptr()) })
        .ok_or_else(|| format!("No application {id}"))?;
    for content_type in content_types(paths) {
        let content_type = text(&content_type);
        let mut error = std::ptr::null_mut();
        // SAFETY: a live application and a type name; a failure is reported, not fatal.
        unsafe {
            let set = if remember { gio.g_app_info_set_as_default_for_type } else { gio.g_app_info_set_as_last_used_for_type };
            if set(app.1, content_type.as_ptr(), &mut error) == 0 && !error.is_null() {
                log::warn!("Cannot record {id} for a file type: {}", failure(gio, error).message);
            }
        }
    }
    let uris: Vec<_> = paths.iter().map(|path| {
        let path = text(path);
        // SAFETY: a path; the file is owned and its URI copied.
        let file = Owned::new(gio, unsafe { (gio.g_file_new_for_path)(path.as_ptr()) });
        file.and_then(|file| unsafe { taken(gio, (gio.g_file_get_uri)(file.1)) })
            .map(|uri| std::ffi::CString::new(uri).unwrap_or_default())
            .unwrap_or_default()
    }).collect();
    // GIO only reads the list: its cells live here.
    let mut cells: Vec<GList> = uris.iter().map(|uri| GList {
        data: uri.as_ptr().cast_mut().cast(),
        next: std::ptr::null_mut(),
        prev: std::ptr::null_mut(),
    }).collect();
    let base = cells.as_mut_ptr();
    for index in 0..cells.len() {
        // SAFETY: indices within `cells`, which is not moved while linked.
        unsafe {
            (*base.add(index)).next = if index + 1 < cells.len() { base.add(index + 1) } else { std::ptr::null_mut() };
            (*base.add(index)).prev = if index > 0 { base.add(index - 1) } else { std::ptr::null_mut() };
        }
    }
    // SAFETY: a new context, released at the end of this call.
    let context = Owned::new(gio, unsafe { (gio.g_app_launch_context_new)() });
    if let (Some(context), Some(variable)) = (&context, crate::platform::linux_gpu::own_variable()) {
        // Applications choose their GPU for themselves, as from the desktop.
        let variable = text(variable);
        // SAFETY: a live context.
        unsafe { (gio.g_app_launch_context_unsetenv)(context.1, variable.as_ptr()) };
    }
    let mut error = std::ptr::null_mut();
    let list = if cells.is_empty() { std::ptr::null_mut() } else { base };
    let context = context.as_ref().map_or(std::ptr::null_mut(), |context| context.1);
    // SAFETY: the application, the list and the context outlive the call; GIO reaps the
    // spawned process itself, or asks a D-Bus activatable application to open the files.
    let launched = unsafe { (gio.g_app_info_launch_uris)(app.1, list, context, &mut error) };
    if launched == 0 {
        // SAFETY: GIO filled the error on failure.
        return Err(if error.is_null() { format!("{id} did not start") } else { unsafe { failure(gio, error) }.message });
    }
    Ok(())
}

#[cfg(test)]
#[path = "applications/tests/icons.rs"]
mod tests;
