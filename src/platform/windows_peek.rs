use super::PeekStatus;
use std::path::PathBuf;

fn executable() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let system = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let roots = local
        .into_iter()
        .flat_map(|root| [root.join("PowerToys"), root.join("Microsoft/PowerToys")])
        .chain(system.into_iter().map(|root| root.join("PowerToys")));
    roots
        .flat_map(|root| {
            [
                root.join("WinUI3Apps/PowerToys.Peek.UI.exe"),
                root.join("modules/Peek/PowerToys.Peek.UI.exe"),
                root.join("PowerToys.Peek.UI.exe"),
            ]
        })
        .find(|path| path.is_file())
}

pub(super) fn peek_status() -> Result<PeekStatus, String> {
    let Some(_) = executable() else {
        return Ok(PeekStatus::default());
    };
    let mut status = PeekStatus {
        installed: true,
        enabled: true,
        activation_shortcut: Some("Ctrl+Space".into()),
        ..Default::default()
    };
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let settings = PathBuf::from(local).join("Microsoft/PowerToys");
        let read = |path| {
            std::fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        };
        if let Some(general) = read(settings.join("settings.json")) {
            status.enabled = general
                .get("enabled")
                .and_then(|enabled| enabled.get("Peek"))
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
        }
        if let Some(peek) = read(settings.join("Peek/settings.json")) {
            apply_settings(&mut status, &peek);
        }
    }
    Ok(status)
}

fn apply_settings(status: &mut PeekStatus, json: &serde_json::Value) {
    let Some(properties) = json.get("properties") else {
        return;
    };
    status.space_enabled = properties
        .get("EnableSpaceToActivate")
        .and_then(|value| value.get("value"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if let Some(shortcut) = properties.get("ActivationShortcut") {
        let shortcut = shortcut.get("value").unwrap_or(shortcut);
        let mut parts = Vec::new();
        for (property, label) in [
            ("ctrl", "Ctrl"),
            ("shift", "Shift"),
            ("alt", "Alt"),
            ("win", "Win"),
        ] {
            if shortcut
                .get(property)
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                parts.push(label.to_string());
            }
        }
        let code = shortcut
            .get("code")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        status.activation_shortcut = virtual_key(code).map(|key| {
            parts.push(key);
            parts.join("+")
        });
    }
}

fn virtual_key(code: u64) -> Option<String> {
    match code {
        0x30..=0x39 | 0x41..=0x5a => char::from_u32(code as u32).map(|key| key.to_string()),
        0x70..=0x87 => Some(format!("F{}", code - 0x6f)),
        0x20 => Some("Space".into()),
        0x0d => Some("Enter".into()),
        0x09 => Some("Tab".into()),
        0x25 => Some("Left".into()),
        0x26 => Some("Up".into()),
        0x27 => Some("Right".into()),
        0x28 => Some("Down".into()),
        _ => None,
    }
}

pub(super) fn open_peek(path: &str) -> Result<(), String> {
    let status = peek_status()?;
    if !status.enabled {
        return Err("PowerToys Peek is disabled".into());
    }
    let exe = executable().ok_or("PowerToys Peek not found")?;
    if !std::path::Path::new(path).is_file() {
        return Err("Peek requires a local file".into());
    }
    // Microsoft documents the CLI as PowerToys.Peek.UI.exe <filepath>.
    let mut command = std::process::Command::new(exe);
    command.arg(path);
    if let Some(parent) = std::path::Path::new(path).parent() {
        command.current_dir(parent);
    }
    command.spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_peek_shortcuts_and_space_option() {
        let mut status = PeekStatus::default();
        apply_settings(
            &mut status,
            &serde_json::json!({"properties": {
                "EnableSpaceToActivate": {"value": true},
                "ActivationShortcut": {"ctrl": true, "shift": true, "code": 80}
            }}),
        );
        assert!(status.space_enabled);
        assert_eq!(status.activation_shortcut.as_deref(), Some("Ctrl+Shift+P"));
        apply_settings(
            &mut status,
            &serde_json::json!({"properties": {"ActivationShortcut": {"value": {"alt": true, "code": 112}}}}),
        );
        assert_eq!(status.activation_shortcut.as_deref(), Some("Alt+F1"));
        apply_settings(
            &mut status,
            &serde_json::json!({"properties": {"ActivationShortcut": {"code": 0}}}),
        );
        assert!(status.activation_shortcut.is_none());
    }
}
