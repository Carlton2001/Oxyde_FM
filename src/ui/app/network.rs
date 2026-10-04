use super::*;

/// A share (or one of its folders), rather than a server or Shell device.
pub(crate) fn mappable_network_path(path: &str) -> bool {
    path.strip_prefix("\\\\").is_some_and(|tail| {
        let mut parts = tail.split('\\');
        parts
            .next()
            .is_some_and(|server| !server.is_empty() && server != "?" && server != ".")
            && parts.next().is_some_and(|share| !share.is_empty())
    })
}

impl OxydeApp {
    pub(crate) fn network_drive_dialog(
        &mut self,
        disconnect: bool,
        path: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.context_menus.clear();
        if self.ejecting_drive {
            cx.notify();
            return;
        }
        self.ejecting_drive = true;
        self.drive_refresh_epoch = self.drive_refresh_epoch.wrapping_add(1);
        self.drive_details_task = None;
        self.sidebar_follow_task = None;
        self.pending_sidebar_requests.clear();
        self.pending_watchers.clear();
        let watchers = if disconnect {
            std::mem::take(&mut self._path_watchers)
        } else {
            std::collections::HashMap::new()
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                for (watched_path, watcher) in watchers {
                    watcher.release(&watched_path)?;
                }
                oxyde_lib::platform::platform().network_drive_dialog(disconnect, path.as_deref())
            })
            .await;
            // Refresh even after cancellation: the native UI may have changed a connection.
            let drives =
                tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().list_drive_roots())
                    .await;
            let _ = this.update(cx, |this, cx| {
                this.ejecting_drive = false;
                let error = match result {
                    Ok(Ok(_)) => None,
                    Ok(Err(error)) => Some(error),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = error {
                    this.file_operation_notice = Some((Message::NetworkDriveFailed, error));
                }
                if let Ok(drives) = drives {
                    this.apply_drive_roots(drives, cx);
                }
                this.request_drive_details(cx);
                this.reconcile_watchers(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::mappable_network_path;
    #[test]
    fn only_unc_shares_can_be_mapped() {
        for path in [
            "\\\\server\\share",
            "\\\\server\\share\\folder",
            "\\\\192.168.1.2\\data$",
        ] {
            assert!(mappable_network_path(path));
        }
        for path in [
            "\\\\server",
            "\\\\server\\",
            "C:\\folder",
            "::{device}",
            "\\\\?\\C:\\",
            "__network_vincinity__",
        ] {
            assert!(!mappable_network_path(path));
        }
    }
}
