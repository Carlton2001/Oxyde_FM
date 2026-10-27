//! UDisks2 client: block devices with a filesystem, the drive that holds them, and the
//! calls that unmount and eject them.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use zbus::blocking::{fdo::ObjectManagerProxy, Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
const SERVICE: &str = "org.freedesktop.UDisks2";

static AVAILABLE: AtomicBool = AtomicBool::new(false);

pub(super) struct Block {
    pub object: String,
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
    pub object: String,
    pub removable: bool,
    pub optical: bool,
    pub usb: bool,
    pub ejectable: bool,
    pub can_power_off: bool,
}

type Properties = HashMap<String, OwnedValue>;

/// Whether the last listing reached UDisks2.
pub(super) fn available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}

fn with_connection<T>(call: impl FnOnce(&Connection) -> zbus::Result<T>) -> zbus::Result<T> {
    static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);
    let connection = {
        let mut slot = CONNECTION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match slot.as_ref() {
            Some(connection) => connection.clone(),
            None => slot.insert(Connection::system()?).clone(),
        }
    };
    let result = call(&connection);
    if matches!(&result, Err(error) if !matches!(error, zbus::Error::MethodError(..) | zbus::Error::FDO(_))) {
        // Reconnect on the next call, in case the bus connection itself failed.
        *CONNECTION.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
    result
}

/// Blocking D-Bus call; run on a background worker.
pub(super) fn blocks() -> zbus::Result<Vec<Block>> {
    let objects = with_connection(|connection| {
        ObjectManagerProxy::builder(connection)
            .destination(SERVICE)?
            .path("/org/freedesktop/UDisks2")?
            .build()?
            .get_managed_objects()
            .map_err(zbus::Error::from)
    });
    AVAILABLE.store(objects.is_ok(), Ordering::Relaxed);
    let objects = objects?;

    let interfaces: HashMap<String, HashMap<&str, &Properties>> = objects
        .iter()
        .map(|(path, interfaces)| {
            let interfaces = interfaces.iter().map(|(name, properties)| (name.as_str(), properties)).collect();
            (path.to_string(), interfaces)
        })
        .collect();
    let drives: HashMap<&str, Drive> = interfaces
        .iter()
        .filter_map(|(path, interfaces)| Some((path.as_str(), drive(path, interfaces.get(DRIVE)?))))
        .collect();
    let mut blocks: Vec<Block> = interfaces
        .iter()
        .filter_map(|(object, interfaces)| {
            let block = interfaces.get(BLOCK)?;
            let drive_path = block.get("Drive").and_then(|value| value.downcast_ref::<ObjectPath>().ok());
            Some(Block {
                object: object.clone(),
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

/// Calls `method` with an empty option map, so that polkit may ask for authentication.
pub(super) fn call(object: &str, interface: &str, method: &str) -> zbus::Result<()> {
    with_connection(|connection| {
        let options: HashMap<&str, Value> = HashMap::new();
        Proxy::new(connection, SERVICE, object, interface)?.call_method(method, &(options,))?;
        Ok(())
    })
}

fn drive(object: &str, properties: &Properties) -> Drive {
    Drive {
        object: object.to_string(),
        removable: boolean(properties, "Removable") || boolean(properties, "MediaRemovable"),
        optical: boolean(properties, "Optical"),
        usb: string(properties, "ConnectionBus") == "usb",
        ejectable: boolean(properties, "Ejectable"),
        can_power_off: boolean(properties, "CanPowerOff"),
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
