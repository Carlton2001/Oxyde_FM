//! Background parsing with bounded publication; the UI only appends ready blocks.
use super::{parse, Content, Markdown, Viewer};
use gpui::*;
use std::{cell::RefCell, collections::HashMap, io::Read, rc::Rc, sync::atomic::Ordering};

impl Viewer {
    pub(in super::super) fn load_markdown(&mut self, path: String, cx: &mut Context<Self>) {
        let generation = self.generation;
        let wanted = self.wanted.clone();
        // At most two batches wait for the UI. Dropping the receiver also
        // releases a producer waiting to publish after a file/mode change.
        let (sender, mut batches) = tokio::sync::mpsc::channel(2);
        let work = crate::ui::preview_jobs::load_viewer(move || {
            let cancelled = || sender.is_closed() || wanted.load(Ordering::Acquire) != generation;
            if cancelled() {
                return Ok(false);
            }
            let file = std::fs::File::open(&path)?;
            let limit = super::super::text::MAX_TEXT_BYTES;
            let truncated = file.metadata()?.len() > limit;
            let mut bytes = Vec::new();
            let mut reader = file.take(limit);
            let mut buffer = [0; 64 * 1024];
            loop {
                if cancelled() {
                    return Ok(truncated);
                }
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            let source = String::from_utf8(bytes)
                .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
            parse::parse_progressively(&source, &path, &cancelled, &mut |batch| {
                !cancelled() && sender.blocking_send(batch).is_ok()
            });
            Ok::<_, std::io::Error>(truncated)
        });
        self.replace_content(Content::Loading, cx);
        let work = cx.background_executor().spawn(work);
        self.loading = Some(cx.spawn(async move |this, cx| {
            let mut first = true;
            let mut published_at = std::time::Instant::now();
            while let Some(batch) = batches.recv().await {
                let current = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    if matches!(this.content, Content::Loading) {
                        this.replace_content(Content::Markdown(Markdown::empty(false)), cx);
                    }
                    let Content::Markdown(document) = &mut this.content else {
                        return false;
                    };
                    document.append_batch(batch);
                    cx.notify();
                    true
                });
                if !matches!(current, Ok(true)) {
                    return;
                }
                // Give the first batch a frame, and keep a fast producer from
                // monopolizing the foreground executor while draining the queue.
                if first || published_at.elapsed() >= std::time::Duration::from_millis(8) {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    first = false;
                    published_at = std::time::Instant::now();
                }
            }
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = None;
                match result {
                    Ok(truncated) => {
                        if let Content::Markdown(document) = &mut this.content {
                            document.truncated = truncated;
                        } else if matches!(this.content, Content::Loading) {
                            this.replace_content(Content::Markdown(Markdown::empty(truncated)), cx);
                        }
                    }
                    Err(error) => {
                        log::warn!("Viewer could not open Markdown: {error}");
                        this.replace_content(Content::Unavailable, cx);
                    }
                }
                cx.notify();
            });
        }));
    }
}

impl Markdown {
    fn empty(truncated: bool) -> Self {
        Self {
            blocks: Rc::new(RefCell::new(Vec::new())),
            list: ListState::new(0, ListAlignment::Top, px(800.0)).measure_all(),
            images: HashMap::new(),
            truncated,
        }
    }

    fn append_batch(&mut self, batch: Vec<super::Block>) {
        let mut blocks = self.blocks.borrow_mut();
        let start = blocks.len();
        let count = batch.len();
        let anchor = self.list.logical_scroll_top();
        blocks.extend(batch);
        self.list.splice(start..start, count);
        // splice does not restart measure_all after its initial layout. Mark
        // just the new rows for measurement, retaining existing row heights.
        self.list.remeasure_items(start..start + count);
        // Also preserve the initial zero offset when inserting into an empty list.
        self.list.scroll_to(anchor);
    }
}

#[cfg(test)]
#[path = "tests/loading.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "tests/native_scroll.rs"]
mod native_tests;
