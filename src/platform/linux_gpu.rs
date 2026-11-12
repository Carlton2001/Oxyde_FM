//! Keeps Vulkan to the GPU the desktop renders with. The loader otherwise opens every
//! installed driver: a sleeping discrete GPU is woken at startup, kept awake while Oxyde
//! runs and woken again to release it at exit.
//!
//! On Wayland the compositor names its GPU, the one GPUI draws with when it can: a window
//! drawn by another GPU may be refused by the compositor and never appear. Without that
//! answer (X11, compositors without linux-dmabuf 4), only GPUs the kernel has put to sleep
//! lose their drivers: the desktop does not render with a sleeping GPU, and waking one
//! would cost power, never a window.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

mod compositor;

const DISABLED: &str = "VK_LOADER_DRIVERS_DISABLE";
/// Through these a user, the desktop ("Launch using Discrete Graphics Card") or a driver
/// chooses the GPU. Any of them leaves the choice untouched.
const USER_CHOICE: &[&str] = &[
    "VK_LOADER_DRIVERS_SELECT", DISABLED, "VK_DRIVER_FILES", "VK_ICD_FILENAMES", "VK_ADD_DRIVER_FILES",
    "MESA_VK_DEVICE_SELECT", "DRI_PRIME", "__NV_PRIME_RENDER_OFFLOAD",
];

static DISABLED_HERE: AtomicBool = AtomicBool::new(false);

/// Vulkan driver manifests of each PCI vendor, as loader globs.
fn drivers(vendor: u16) -> &'static [&'static str] {
    match vendor {
        0x10de => &["*nvidia*", "*nouveau*"],
        0x1002 => &["*radeon*", "*amd*"],
        0x8086 => &["*intel*"],
        _ => &[],
    }
}

#[derive(Debug, PartialEq)]
struct Card {
    name: String,
    vendor: u16,
    /// The PCI device, resolved: the key shared with the compositor's device node.
    device: PathBuf,
    asleep: bool,
}

fn read_cards(drm: &Path) -> Vec<Card> {
    let Ok(entries) = fs::read_dir(drm) else { return Vec::new() };
    let mut cards: Vec<Card> = entries.flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_prefix("card").is_some_and(|number| number.parse::<u32>().is_ok()).then_some(name)
        })
        .filter_map(|name| {
            let device = fs::canonicalize(drm.join(&name).join("device")).ok()?;
            let vendor = fs::read_to_string(device.join("vendor")).ok()?;
            let vendor = u16::from_str_radix(vendor.trim().trim_start_matches("0x"), 16).ok()?;
            let asleep = fs::read_to_string(device.join("power/runtime_status")).is_ok_and(|status| status.trim() == "suspended");
            Some(Card { name, vendor, device, asleep })
        })
        .collect();
    cards.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    cards
}

/// The card of the device node `node` (`major:minor`), which may be a render node.
fn card_of_node<'a>(cards: &'a [Card], dev_char: &Path, node: &str) -> Option<&'a Card> {
    let device = fs::canonicalize(dev_char.join(node).join("device")).ok()?;
    cards.iter().find(|card| card.device == device)
}

/// Each card's vendor and whether its drivers must stay: only the compositor's when it is
/// known, otherwise every GPU that is awake.
fn needed(cards: &[Card], rendering: Option<&Card>) -> Vec<(u16, bool)> {
    cards.iter()
        .map(|card| (card.vendor, rendering.map_or(!card.asleep, |chosen| chosen.device == card.device)))
        .collect()
}

/// Globs of the drivers of vendors none of whose cards is needed, or `None` when nothing
/// is known to be safe to disable. A vendor's drivers serve all its cards.
fn idle_drivers(gpus: &[(u16, bool)]) -> Option<String> {
    let kept: BTreeSet<u16> = gpus.iter().filter(|gpu| gpu.1).map(|gpu| gpu.0).collect();
    // No GPU known to be in use (sysfs hidden, headless session): change nothing.
    if kept.is_empty() {
        return None;
    }
    let idle: BTreeSet<u16> = gpus.iter().map(|gpu| gpu.0).filter(|vendor| !kept.contains(vendor)).collect();
    let globs: Vec<&str> = idle.into_iter().flat_map(drivers).copied().collect();
    (!globs.is_empty()).then(|| globs.join(","))
}

/// Must run while the process has a single thread. Returns the disabled drivers and why,
/// for the log.
pub(super) fn prefer_display_gpu() -> Option<String> {
    if USER_CHOICE.iter().any(|name| std::env::var_os(name).is_some()) {
        return None;
    }
    let cards = read_cards(Path::new("/sys/class/drm"));
    // One vendor leaves nothing to choose: the compositor need not be asked.
    let vendors: BTreeSet<u16> = cards.iter().map(|card| card.vendor).filter(|vendor| !drivers(*vendor).is_empty()).collect();
    if vendors.len() < 2 {
        return None;
    }
    let rendering = compositor::main_device().and_then(|node| card_of_node(&cards, Path::new("/sys/dev/char"), &node));
    let globs = idle_drivers(&needed(&cards, rendering))?;
    std::env::set_var(DISABLED, &globs);
    DISABLED_HERE.store(true, Ordering::Relaxed);
    Some(match rendering {
        Some(card) => format!("{globs} (the compositor renders with {})", card.name),
        None => format!("{globs} (asleep; the compositor did not name its GPU)"),
    })
}

/// The variable `leave_out_of` removes, when Oxyde set it for itself.
pub(super) fn own_variable() -> Option<&'static str> {
    DISABLED_HERE.load(Ordering::Relaxed).then_some(DISABLED)
}

/// Applications Oxyde opens choose their GPU for themselves, as if opened by the desktop.
pub(super) fn leave_out_of(command: &mut Command) {
    if DISABLED_HERE.load(Ordering::Relaxed) {
        command.env_remove(DISABLED);
    }
}

#[cfg(test)]
#[path = "linux_gpu/tests/drivers.rs"]
mod tests;
