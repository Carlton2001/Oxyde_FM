use super::OxydeApp;
use gpui::*;

impl OxydeApp {
    /// Entry origin was resolved by listing/search workers, never by UI rendering.
    pub(crate) fn file_for_action(&self, path: &str, is_dir: bool) -> oxyde_lib::models::FileEntry {
        let mut found = None;
        self.state.layout.visit_panes(&mut |pane| {
            if found.is_some() { return; }
            for tab in &pane.tabs {
                if let Some(file) = tab.files.iter().find(|file| file.path == path) {
                    found = Some(file.clone());
                    return;
                }
            }
        });
        found.unwrap_or_else(|| oxyde_lib::models::FileEntry {
            path: path.into(), is_dir, ..Default::default()
        })
    }

    pub(super) fn archive_qualities(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([
            ("zip".into(), self.config.zip_quality.clone()),
            ("7z".into(), self.config.seven_zip_quality.clone()),
            ("zst".into(), self.config.zstd_quality.clone()),
        ])
    }

    pub(super) fn archive_quality_for_path(&self, path: &std::path::Path) -> String {
        let mut archive = self.file_for_action(&path.to_string_lossy(), false).archive_path
            .map(|root| root.as_ref().clone());
        self.state.layout.visit_panes(&mut |pane| {
            for tab in &pane.tabs {
                if let Some(root) = &tab.archive_root {
                    if path.starts_with(root) { archive = Some(root.clone()); }
                }
            }
        });
        let archive = archive.unwrap_or_else(|| path.to_path_buf());
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.state.layout.find_pane(&self.state.active_pane_id) else {
            return;
        };
        let tab = pane.active_tab();
        let paths = compression_sources(tab.files.iter().filter(|file| tab.selected_paths.contains(&file.path)).cloned());
        let Some(first) = paths.first() else {
            return;
        };
        let format = self.default_archive_format().to_string();
        let extension = if format == "zst" { "tar.zst" } else { &format };
        let destination = first.with_extension(extension);
        self.begin_rename_with_extension(
            destination.to_string_lossy().into_owned(),
            false,
            false,
            Some(extension),
            window,
            cx,
        );
        self.archive_creation = Some((paths, extension.to_string()));
    }

    pub(crate) fn default_archive_format(&self) -> &str {
        match self.config.default_archive_format.as_str() {
            "7z" => "7z", "tar" => "tar", "zst" => "zst", _ => "zip",
        }
    }
    pub(crate) fn set_default_archive_format(&mut self, format: &str, cx: &mut Context<Self>) {
        self.config.default_archive_format = format.into();
        self.save_preferences(cx);
        cx.notify();
    }
    pub(crate) fn select_archive_format(&mut self, format: &str, cx: &mut Context<Self>) {
        let Some((_, extension)) = self.archive_creation.as_mut() else { return; };
        let next = if format == "zst" { "tar.zst" } else { &format };
        self.editing_text = archive_name_for_format(&self.editing_text, extension, next);
        *extension = next.into();
        if let Some(input) = &self.editing_view {
            input.update(cx, |input, cx| input.set_content(self.editing_text.clone(), cx));
        }
        self.rename_error = None;
        cx.notify();
    }

    pub fn execute_extract_archive(&mut self, path: String, cx: &mut Context<Self>) {
        let files = self.context_action_paths(&path).iter().map(|path| self.file_for_action(path, false)).collect();
        for (source, destination) in extraction_jobs(files) {
            self.queue_archive_operation(
                oxyde_lib::systems::file_ops::FileOpType::Extract,
                vec![source],
                destination,
                cx,
            );
        }
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
                    operation.set_turbo(this.config.default_turbo_mode);
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

fn compression_sources(paths: impl IntoIterator<Item = oxyde_lib::models::FileEntry>) -> Vec<std::path::PathBuf> {
    let mut sources: Vec<_> = paths.into_iter()
        .filter(|file| file.archive_path.is_none())
        .map(|file| std::path::PathBuf::from(file.path)).collect();
    sources.sort();
    sources
}

fn extraction_jobs(paths: Vec<oxyde_lib::models::FileEntry>) -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    paths.into_iter().filter_map(|file| {
        if !file.is_navigable_archive() {
            return None;
        }
        let source = std::path::PathBuf::from(file.path);
        let parent = source.parent()?;
        let name = source.file_name().unwrap_or_default().to_string_lossy();
        let stem = [".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2"]
            .iter()
            .find_map(|suffix| name.strip_suffix(suffix))
            .or_else(|| source.file_stem().and_then(|stem| stem.to_str()))
            .unwrap_or("archive");
        let destination = parent.join(stem);
        Some((source, destination))
    }).collect()
}

#[cfg(test)]
#[path = "archives/tests/extraction.rs"]
mod extraction_tests;

fn archive_name_for_format(name: &str, previous: &str, next: &str) -> String {
    let suffix = format!(".{previous}");
    let stem = if name.to_ascii_lowercase().ends_with(&suffix) {
        &name[..name.len() - suffix.len()]
    } else { name };
    format!("{stem}.{next}")
}
#[cfg(test)]
#[path = "archives/tests/creation.rs"]
mod creation_tests;
