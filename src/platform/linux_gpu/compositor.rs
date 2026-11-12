//! Asks the Wayland compositor which GPU it renders with: the main device of the
//! linux-dmabuf default feedback (version 4). GPUI ranks that GPU first, and Mesa's
//! Vulkan device selection reads the same answer.
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_feedback_v1::{self, ZwpLinuxDmabufFeedbackV1};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::{self, ZwpLinuxDmabufV1};

#[derive(Default)]
struct Feedback {
    main_device: Option<Vec<u8>>,
}

impl Dispatch<WlRegistry, GlobalListContents> for Feedback {
    fn event(_: &mut Self, _: &WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpLinuxDmabufV1, ()> for Feedback {
    fn event(_: &mut Self, _: &ZwpLinuxDmabufV1, _: zwp_linux_dmabuf_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpLinuxDmabufFeedbackV1, ()> for Feedback {
    fn event(
        state: &mut Self, _: &ZwpLinuxDmabufFeedbackV1, event: zwp_linux_dmabuf_feedback_v1::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>,
    ) {
        if let zwp_linux_dmabuf_feedback_v1::Event::MainDevice { device } = event {
            state.main_device = Some(device);
        }
    }
}

/// The `major:minor` number of the compositor's render device, or `None` outside
/// Wayland and when the compositor does not say.
pub(super) fn main_device() -> Option<String> {
    // A socket handed down by the parent can be connected to only once: it is GPUI's.
    if std::env::var_os("WAYLAND_SOCKET").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return None;
    }
    let connection = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<Feedback>(&connection).ok()?;
    let handle = queue.handle();
    let dmabuf: ZwpLinuxDmabufV1 = globals.bind(&handle, 4..=4, ()).ok()?;
    let feedback = dmabuf.get_default_feedback(&handle, ());
    let mut answer = Feedback::default();
    // The feedback is sent at once, before the reply to the roundtrip's sync.
    queue.roundtrip(&mut answer).ok()?;
    feedback.destroy();
    dmabuf.destroy();
    let bytes = answer.main_device?.try_into().ok()?;
    let device = libc::dev_t::from_ne_bytes(bytes);
    Some(format!("{}:{}", libc::major(device), libc::minor(device)))
}
