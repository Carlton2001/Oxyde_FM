//! UDisks2 client: block devices with a filesystem and the drive that holds them.
use std::collections::HashMap;
use std::sync::Mutex;
use zbus::blocking::{fdo::ObjectManagerProxy, Connection};
use zbus::zvariant::{ObjectPath, OwnedValue};

const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";

pub(super) struct Block {
    pub device: String,
    pub label: String,
    /// UDisks2 `IdType` and `IdVersion`, such as `vfat` and `FAT32`.
    pub id_type: String,
    pub id_version: String,
    pub hint_ignore: bool,
    /// `None` without a filesystem; empty when the filesystem is not mounted.
    pub mount_points: Option<Vec<String>>,
    pub drive: Option<Drive>,
}

#[derive(Clone)]
pub(super) struct Drive {
    pub removable: bool,
    pub optical: bool,
    pub usb: bool,
}

type Properties = HashMap<String, OwnedValue>;

/// Blocking D-Bus call; run on a background worker.
pub(super) fn blocks() -> zbus::Result<Vec<Block>> {
    static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);
    let mut slot = CONNECTION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let connection = match slot.as_ref() {
        Some(connection) => connection.clone(),
        None => slot.insert(Connection::system()?).clone(),
    };
    let objects = ObjectManagerProxy::builder(&connection)
        .destination("org.freedesktop.UDisks2")?
        .path("/org/freedesktop/UDisks2")?
        .build()
        .and_then(|proxy| proxy.get_managed_objects().map_err(zbus::Error::from));
    let objects = match objects {
        Ok(objects) => objects,
        Err(error) => {
            // Reconnect on the next call, in case the bus connection itself failed.
            *slot = None;
            return Err(error);
        }
    };
    drop(slot);

    let interfaces: HashMap<String, HashMap<&str, &Properties>> = objects
        .iter()
        .map(|(path, interfaces)| {
            let interfaces = interfaces.iter().map(|(name, properties)| (name.as_str(), properties)).collect();
            (path.to_string(), interfaces)
        })
        .collect();
    let drives: HashMap<&str, Drive> = interfaces
        .iter()
        .filter_map(|(path, interfaces)| Some((path.as_str(), drive(interfaces.get(DRIVE)?))))
        .collect();
    let mut blocks: Vec<Block> = interfaces
        .values()
        .filter_map(|interfaces| {
            let block = interfaces.get(BLOCK)?;
            let drive_path = block.get("Drive").and_then(|value| value.downcast_ref::<ObjectPath>().ok());
            Some(Block {
                device: bytes_string(block.get("PreferredDevice")).unwrap_or_default(),
                label: string(block, "IdLabel"),
                id_type: string(block, "IdType"),
                id_version: string(block, "IdVersion"),
                hint_ignore: boolean(block, "HintIgnore"),
                mount_points: interfaces.get(FILESYSTEM).map(|filesystem| mount_points(filesystem)),
                drive: drive_path.and_then(|path| drives.get(path.as_str()).cloned()),
            })
        })
        .collect();
    blocks.sort_by(|a, b| a.device.cmp(&b.device));
    Ok(blocks)
}

fn drive(properties: &Properties) -> Drive {
    Drive {
        removable: boolean(properties, "Removable") || boolean(properties, "MediaRemovable"),
        optical: boolean(properties, "Optical"),
        usb: string(properties, "ConnectionBus") == "usb",
    }
}

fn boolean(properties: &Properties, name: &str) -> bool {
    properties.get(name).and_then(|value| value.downcast_ref::<bool>().ok()).unwrap_or(false)
}

fn string(properties: &Properties, name: &str) -> String {
    properties
        .get(name)
        .and_then(|value| value.downcast_ref::<&str>().ok().map(str::to_owned))
        .unwrap_or_default()
}

/// UDisks2 sends paths as NUL-terminated byte arrays (`ay`).
fn bytes_string(value: Option<&OwnedValue>) -> Option<String> {
    let bytes = Vec::<u8>::try_from(value?.try_clone().ok()?).ok()?;
    Some(String::from_utf8_lossy(bytes.strip_suffix(&[0]).unwrap_or(&bytes)).into_owned())
}

fn mount_points(filesystem: &Properties) -> Vec<String> {
    filesystem
        .get("MountPoints")
        .and_then(|value| Vec::<Vec<u8>>::try_from(value.try_clone().ok()?).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|bytes| String::from_utf8_lossy(bytes.strip_suffix(&[0]).unwrap_or(&bytes)).into_owned())
        .collect()
}
