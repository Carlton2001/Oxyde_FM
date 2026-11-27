use crate::ui::{i18n::Message, theme::OxydeTheme};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::CloudStatus;

const ICON_SIZE: Rems = rems(0.75);

pub(crate) fn cloud_status_icon(status: CloudStatus, theme: &OxydeTheme) -> Div {
    sized_cloud_status_icon(status, theme, ICON_SIZE)
}

pub(super) fn sized_cloud_status_icon(status: CloudStatus, theme: &OxydeTheme, size: Rems) -> Div {
    let (path, color) = match status {
        CloudStatus::OnlineOnly => ("icons/cloud.svg", theme.accent),
        CloudStatus::LocallyAvailable => ("icons/circle-check.svg", theme.success_text),
        CloudStatus::AlwaysAvailable => ("icons/check.svg", theme.active_background),
        CloudStatus::SyncPending | CloudStatus::Syncing => ("icons/refresh-cw.svg", theme.accent),
    };
    let pinned = status == CloudStatus::AlwaysAvailable;
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .when(pinned, |icon| icon.rounded_full().bg(theme.success_text))
        .child(
            svg()
                .path(path)
                .size(if pinned { rems(size.0 * 5.0 / 6.0) } else { size })
                .text_color(color),
        )
}

pub(crate) fn cloud_status_message(status: CloudStatus) -> Message {
    match status {
        CloudStatus::OnlineOnly => Message::CloudStatusOnlineOnly,
        CloudStatus::LocallyAvailable => Message::CloudStatusLocallyAvailable,
        CloudStatus::AlwaysAvailable => Message::CloudStatusAlwaysAvailable,
        CloudStatus::SyncPending => Message::CloudStatusSyncPending,
        CloudStatus::Syncing => Message::CloudStatusSyncing,
    }
}
