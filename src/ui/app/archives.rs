use super::OxydeApp;
use gpui::*;

impl OxydeApp {
    pub(super) fn archive_qualities(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([
            ("zip".into(), self.config.zip_quality.clone()),
            ("7z".into(), self.config.seven_zip_quality.clone()),
            ("zst".into(), self.config.zstd_quality.clone()),
        ])
    }

    pub(super) fn archive_quality_for_path(&self, path: &std::path::Path) -> String {
        let archive = oxyde_lib::utils::archive::split_virtual_path(&path.to_string_lossy())
            .map(|(archive, _)| archive)
            .unwrap_or_else(|| path.to_path_buf());
        match archive
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("7z") => self.config.seven_zip_quality.clone(),
            Some("zst") => self.config.zstd_quality.clone(),
            _ => self.config.zip_quality.clone(),
        }
    }

    pub fn execute_compress_selected(
        &mut self,
        format: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.state.layout.find_pane(&self.state.active_pane_id) else {
            return;
        };
        let paths: Vec<std::path::PathBuf> = pane
            .active_tab()
            .selected_paths
            .iter()
            .map(std::path::PathBuf::from)
            .filter(|path| {
                oxyde_lib::utils::archive::split_virtual_path(&path.to_string_lossy()).is_none()
            })
            .collect();
        let Some(first) = paths.first() else {
            return;
        };
        let extension = if format == "zst" { "tar.zst" } else { format };
        let destination = first.with_extension(extension);
        self.begin_rename(
            destination.to_string_lossy().into_owned(),
            false,
            false,
            window,
            cx,
        );
        self.archive_creation = Some((paths, extension.to_string()));
    }

    pub fn execute_extract_archive(&mut self, path: String, cx: &mut Context<Self>) {
        let source = std::path::PathBuf::from(path);
        let Some(parent) = source.parent() else {
            return;
        };
        let name = source.file_name().unwrap_or_default().to_string_lossy();
        let stem = [".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2"]
            .iter()
            .find_map(|suffix| name.strip_suffix(suffix))
            .or_else(|| source.file_stem().and_then(|stem| stem.to_str()))
            .unwrap_or("archive");
        let destination = parent.join(stem);
        self.queue_archive_operation(
            oxyde_lib::systems::file_ops::FileOpType::Extract,
            vec![source],
            destination,
            cx,
        );
    }

    pub(super) fn queue_archive_operation(
        &mut self,
        kind: oxyde_lib::systems::file_ops::FileOpType,
        sources: Vec<std::path::PathBuf>,
        destination: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                handle.update(&mut cx, |this, cx| {
                    let mut operation = oxyde_lib::systems::file_ops::FileOperation::new(
                        kind,
                        sources,
                        Some(destination),
                        None,
                    );
                    operation.compression_quality =
                        this.archive_quality_for_path(operation.destination.as_ref().unwrap());
                    operation.archive_qualities = this.archive_qualities();
                    let id =
                        this.file_ops
                            .queue_operation_on(&runtime, this.history.clone(), operation);
                    this.watch_file_operation(id, cx);
                });
            }
        })
        .detach();
    }
}
