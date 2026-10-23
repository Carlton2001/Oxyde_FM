//! One self-worker per Viewer PDF keeps the document open, as browsers do, so a
//! page far into a long document renders without reloading it. Requests are a
//! page index (u32 LE); replies are length-prefixed preview-protocol frames.
use super::windows_preview_worker::Worker;
use crate::models::CommandError;
use crate::utils::preview_protocol::{self, Reply};
use std::{
    io::{self, Read, Write},
    os::windows::io::AsRawHandle,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
use windows::Win32::{Foundation::HANDLE, System::Pipes::PeekNamedPipe};

/// Native PDF rendering stays within the still-preview job size.
const MEMORY_BYTES: usize = 1024 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(15);
const CLOSE_DEADLINE: Duration = Duration::from_secs(2);
const CLOSE_DOCUMENT: u32 = u32::MAX;
const DOCUMENT_CLOSED: &str = "pdf-document-closed";
mod previews;

pub(super) fn run_previews() -> io::Result<()> { previews::run() }

fn frame(reply: Reply, output: &mut impl Write) -> io::Result<()> {
    let mut bytes = Vec::new();
    preview_protocol::encode(reply, &mut bytes)?;
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}

fn error_reply(error: CommandError) -> Reply {
    Reply::Error { limit: false, detail: error.to_string() }
}

/// Worker side: page sizes first, then one frame per requested page.
pub(super) fn run(path: &Path) -> io::Result<()> {
    let _com = super::windows_com::ComApartment::mta_or_existing().map_err(io::Error::other)?;
    let mut output = io::stdout().lock();
    let path = path.to_str().ok_or_else(|| io::Error::other("PDF path is not Unicode"))?;
    let document = match super::windows_pdf_thumbnail::open(path) {
        Ok(document) => document,
        Err(error) => return frame(error_reply(error), &mut output),
    };
    let sizes = super::windows_pdf_thumbnail::sizes_of(&document);
    let opened = sizes.is_ok();
    frame(sizes.map(Reply::Text).unwrap_or_else(error_reply), &mut output)?;
    if !opened {
        return Ok(());
    }
    let mut input = io::stdin().lock();
    let mut request = [0; 4];
    while input.read_exact(&mut request).is_ok() {
        let index = u32::from_le_bytes(request);
        if index == CLOSE_DOCUMENT {
            // A normal COM release precedes the receipt. Killing the worker is
            // only a fallback, rather than the document-close protocol.
            drop(document);
            frame(Reply::Text(DOCUMENT_CLOSED.into()), &mut output)?;
            return Ok(());
        }
        let reply = super::windows_pdf_thumbnail::render_page(
            &document,
            index,
            super::windows_pdf_thumbnail::VIEWER_SIDE,
        )
        .map(|image| Reply::Images { frames: vec![image::Frame::new(image)], limited: false, natural_size: None })
        .unwrap_or_else(error_reply);
        frame(reply, &mut output)?;
    }
    Ok(())
}

pub(super) struct Session {
    worker: Option<Worker>,
    output: std::fs::File,
    input: std::process::ChildStdin,
    sizes: Vec<(f32, f32)>,
}

impl Session {
    pub(super) fn open(path: &Path) -> Result<Self, CommandError> {
        Self::open_in(&std::env::current_exe()?, path)
    }

    fn open_in(executable: &Path, path: &Path) -> Result<Self, CommandError> {
        let mut command = Command::new(executable);
        command.arg("--oxyde-preview-worker-v1").arg("pdf-session").arg(path);
        let (worker, output, input) = Worker::start_duplex_checked(command, MEMORY_BYTES, &|| Ok(()))?;
        let mut session = Self { worker: Some(worker), output, input, sizes: Vec::new() };
        let text = match session.reply(DEADLINE)? {
            Reply::Text(text) => text,
            Reply::Error { detail, .. } => return Err(CommandError::Other(detail)),
            _ => return Err(CommandError::Other("Unexpected PDF session reply".into())),
        };
        session.sizes = super::thumbnails::parse_pdf_sizes(&text)?;
        Ok(session)
    }

    /// Reads one frame, giving up when the worker stalls or exits.
    fn reply(&mut self, deadline: Duration) -> Result<Reply, CommandError> {
        let started = Instant::now();
        let mut header = [0; 4];
        self.read_exact(&mut header, started, deadline)?;
        let length = u32::from_le_bytes(header) as usize;
        if length == 0 || length > preview_protocol::MAX_REPLY {
            return Err(CommandError::Other("Invalid PDF session reply".into()));
        }
        let mut bytes = vec![0; length];
        self.read_exact(&mut bytes, started, deadline)?;
        Ok(preview_protocol::decode(&bytes)?)
    }

    fn read_exact(&mut self, mut buffer: &mut [u8], started: Instant, deadline: Duration) -> io::Result<()> {
        while !buffer.is_empty() {
            let mut available = 0;
            unsafe {
                PeekNamedPipe(HANDLE(self.output.as_raw_handle()), None, 0, None, Some(&mut available), None)
            }
            .map_err(io::Error::other)?;
            if available == 0 {
                let worker = self.worker.as_mut().ok_or_else(|| io::Error::other("PDF session is closed"))?;
                if worker.child.try_wait()?.is_some() {
                    return Err(io::Error::other("PDF session worker exited"));
                }
                if started.elapsed() > deadline {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "PDF session response exceeded its deadline"));
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            let count = (available as usize).min(buffer.len());
            let read = self.output.read(&mut buffer[..count])?;
            buffer = &mut buffer[read..];
        }
        Ok(())
    }
}

impl super::PdfSession for Session {
    fn close(&mut self) -> Result<(), CommandError> {
        if self.worker.is_none() { return Ok(()); }
        let result = (|| {
            self.input.write_all(&CLOSE_DOCUMENT.to_le_bytes())?;
            self.input.flush()?;
            match self.reply(CLOSE_DEADLINE)? {
                Reply::Text(text) if text == DOCUMENT_CLOSED => Ok(()),
                _ => Err(CommandError::Other("Missing PDF close receipt".into())),
            }
        })();
        if result.is_ok() {
            if let Some(worker) = &self.worker {
                // Give the worker its normal exit after releasing COM. A failed
                // handshake/exit still uses Worker's bounded kill-and-reap path.
                unsafe { windows::Win32::System::Threading::WaitForSingleObject(
                    HANDLE(worker.child.as_raw_handle()), 1000,
                ); }
            }
        }
        drop(self.worker.take());
        result
    }

    fn sizes(&self) -> &[(f32, f32)] {
        &self.sizes
    }

    fn page(&mut self, index: u32) -> Result<image::RgbaImage, CommandError> {
        if self.worker.is_none() || index == CLOSE_DOCUMENT {
            return Err(CommandError::Other("PDF session is closed or page is out of range".into()));
        }
        self.input.write_all(&index.to_le_bytes())?;
        self.input.flush()?;
        match self.reply(DEADLINE)? {
            Reply::Images { mut frames, .. } if frames.len() == 1 => Ok(frames.remove(0).into_buffer()),
            Reply::Error { detail, .. } => Err(CommandError::Other(detail)),
            _ => Err(CommandError::Other("Unexpected PDF session reply".into())),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Also cover an opening result discarded before publication in the UI.
        let _ = super::PdfSession::close(self);
    }
}

#[cfg(test)]
#[path = "windows_pdf_session/tests/recycle.rs"]
mod recycle_tests;

#[cfg(test)]
mod tests {
    use super::super::PdfSession;
    use super::*;

    pub(super) fn write_pdf(path: &Path) {
        // Two red pages: 72x144 then 144x72 points.
        let pages = [(72, 144), (144, 72)];
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".to_string(),
        ];
        for (index, (width, height)) in pages.iter().enumerate() {
            let content = format!("1 0 0 rg 0 0 {width} {height} re f\n");
            objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << >> /Contents {} 0 R >>", 4 + index * 2));
            objects.push(format!("<< /Length {} >>\nstream\n{content}endstream", content.len()));
        }
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
        );
        std::fs::write(&path, pdf).unwrap();
    }

    #[test]
    fn session_keeps_the_document_open_across_page_requests() {
        let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde.exe");
        assert!(executable.is_file(), "Build oxyde.exe before native worker tests");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("two.pdf");
        write_pdf(&path);

        let mut session = Session::open_in(&executable, &path).unwrap();
        assert_eq!(session.sizes().len(), 2);
        let last = session.page(1).unwrap();
        assert!(last.width() > last.height());
        let first = session.page(0).unwrap();
        assert!(first.height() > first.width());
        assert!(session.page(2).is_err());
        assert!(session.page(0).is_ok(), "A page error must not close the session");
        drop(session);
        std::fs::remove_file(&path).expect("Closing the native PDF session must release its source file");
    }
}
