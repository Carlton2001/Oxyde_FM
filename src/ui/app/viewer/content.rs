//! Complete file content for the Viewer: images, every PDF page (see `pdf`),
//! Office and OpenDocument files and whole text files (see `text`). Decoding
//! stays in workers or background jobs.
use super::Viewer;
use crate::ui::components::{icons::PreviewType, tooltip::PreviewTask};
use gpui::*;
use std::sync::Arc;

const LIST_OVERDRAW: Pixels = px(800.0);
/// Office Open XML and OpenDocument formats rendered as complete documents.
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "xltx", "xltm", "pptx", "pptm", "potx",
    "potm", "ppsx", "ppsm", "odt", "ott", "ods", "ots", "odp", "otp",
];

pub(super) enum Content {
    None,
    Loading,
    Unavailable,
    Image(Arc<RenderImage>),
    Audio(super::audio::Audio),
    Pdf(super::pdf::Pdf),
    Document(Arc<Vec<Block>>, ListState),
    Text(super::text::TextFile),
    Markdown(super::markdown::Markdown),
    /// Formats without a complete renderer keep the existing preview.
    Preview(Entity<PreviewTask>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockKind {
    Title,
    Heading(u8),
    Paragraph,
    Item,
    Row,
    Sheet,
    Slide,
}

pub(super) struct Block {
    pub(super) kind: BlockKind,
    pub(super) text: SharedString,
    pub(super) cells: Vec<SharedString>,
}

fn extension(name: &str) -> String {
    name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default()
}

pub(super) fn parse_document(text: &str) -> Vec<Block> {
    text.lines()
        .filter_map(|line| {
            let (tag, text) = line.split_once('\t')?;
            let kind = match tag {
                "title" => BlockKind::Title,
                "h1" => BlockKind::Heading(1),
                "h2" => BlockKind::Heading(2),
                "h3" => BlockKind::Heading(3),
                "h4" => BlockKind::Heading(4),
                "li" => BlockKind::Item,
                "tr" => BlockKind::Row,
                "sheet" => BlockKind::Sheet,
                "slide" => BlockKind::Slide,
                _ => BlockKind::Paragraph,
            };
            let cells = if kind == BlockKind::Row {
                text.split(oxyde_lib::utils::office_document::CELL_SEPARATOR)
                    .map(|cell| SharedString::from(cell.to_owned()))
                    .collect()
            } else {
                Vec::new()
            };
            Some(Block { kind, text: text.to_owned().into(), cells })
        })
        .collect()
}

impl Content {
    fn images(&self) -> Vec<Arc<RenderImage>> {
        match self {
            Self::Image(image) => vec![image.clone()],
            Self::Audio(audio) => audio.cover.iter().cloned().collect(),
            Self::Pdf(pdf) => pdf.images().collect(),
            Self::Markdown(document) => document.images(),
            _ => Vec::new(),
        }
    }
}

/// Retire displayed pixels after GPUI restores the window currently being updated.
pub(super) fn retire_images(images: Vec<Arc<RenderImage>>, cx: &mut App) {
    if images.is_empty() { return; }
    for image in &images {
        crate::ui::memory_diagnostics::track(image, crate::ui::memory_diagnostics::State::Retired);
    }
    // During a window callback GPUI temporarily removes that window from
    // App.windows. None would silently skip its atlas without this deferral.
    cx.defer(move |cx| {
        for image in images { cx.drop_image(image, None); }
    });
}

impl Viewer {
    /// Release GPU textures with the content that owned them.
    pub(super) fn replace_content(&mut self, content: Content, cx: &mut Context<Self>) {
        let old = std::mem::replace(&mut self.content, content);
        if crate::ui::memory_diagnostics::enabled() {
            use crate::ui::memory_diagnostics::{track, State};
            for image in self.content.images() { track(&image, State::Shown); }
            crate::ui::memory_diagnostics::checkpoint("viewer-content-replaced");
        }
        retire_images(old.images(), cx);
        if let Content::Pdf(pdf) = old {
            pdf.retire(&mut self.jobs, cx);
        }
    }

    pub(super) fn load_content(&mut self, file: &oxyde_lib::models::FileEntry, cx: &mut Context<Self>) {
        use crate::ui::components::icons::get_preview_type;
        let path = file.path.clone();
        let generation = self.generation;
        if super::markdown::is_markdown(&path) {
            if self.markdown_rendered { self.load_markdown(path, cx); }
            else { self.load_text(path, cx); }
            return;
        }
        let kind = if DOCUMENT_EXTENSIONS.contains(&extension(&file.name).as_str()) {
            None
        } else {
            Some(get_preview_type(&file.name))
        };
        let Some(kind) = kind else {
            let work = self.load_work(move || {
                oxyde_lib::platform::platform().office_document(path)
            });
            self.finish(
                generation,
                work,
                |text| {
                    let blocks = parse_document(&text);
                    // Measured once per width: the rail keeps its size while scrolling.
                    let list = ListState::new(blocks.len(), ListAlignment::Top, LIST_OVERDRAW).measure_all();
                    Content::Document(Arc::new(blocks), list)
                },
                cx,
            );
            return;
        };
        match kind {
            PreviewType::Image => {
                let renderer = cx.svg_renderer();
                // Browsing quickly queues requests; skip those for files left
                // before their turn instead of decoding them for nothing.
                let wanted = self.wanted.clone();
                wanted.store(generation, std::sync::atomic::Ordering::Release);
                let work = self.load_work(move || {
                    if wanted.load(std::sync::atomic::Ordering::Acquire) != generation {
                        return Err("Viewer image no longer shown".to_string());
                    }
                    crate::ui::image_preview::load_viewer(std::path::Path::new(&path), &renderer)
                        .map_err(|error| error.to_string())
                });
                self.finish(generation, work, |preview| Content::Image(preview.image), cx);
            }
            PreviewType::Pdf => self.load_pdf(path, cx),
            PreviewType::Text => self.load_text(path, cx),
            PreviewType::None => self.replace_content(Content::Unavailable, cx),
            kind => {
                let preview = cx.new(|cx| PreviewTask::new_viewer(path, kind, cx));
                cx.observe(&preview, |_, _, cx| cx.notify()).detach();
                self.replace_content(Content::Preview(preview), cx);
            }
        }
    }

    pub(super) fn finish<T: Send + 'static>(
        &mut self,
        generation: u64,
        work: impl std::future::Future<Output = Result<(T, futures::channel::oneshot::Sender<()>), String>> + Send + 'static,
        build: impl FnOnce(T) -> Content + 'static,
        cx: &mut Context<Self>,
    ) {
        self.replace_content(Content::Loading, cx);
        self.loading = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = None;
                let content = match result {
                    Ok((value, _completion)) => build(value),
                    Err(error) => {
                        log::warn!("Viewer could not open the file: {error}");
                        Content::Unavailable
                    }
                };
                this.replace_content(content, cx);
                cx.notify();
            });
        }));
    }
}

#[cfg(test)]
#[path = "tests/content.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "tests/native_image_cleanup.rs"]
mod native_image_cleanup;

#[cfg(all(test, target_os = "windows"))]
#[path = "tests/native_graphics_memory.rs"]
mod native_graphics_memory;
