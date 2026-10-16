//! Canonical decomposition with bounded reordering storage. Only an unusually
//! long combining sequence spills to a private, automatically removed file.
use super::vm::Vm;
use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    sync::atomic::{AtomicBool, Ordering},
};
use unicode_normalization::char::{canonical_combining_class, decompose_canonical};

const MARK_LIMIT: usize = 4096;

struct Spill {
    writer: BufWriter<File>,
    classes: [bool; 256],
}

impl Spill {
    fn push(&mut self, class: u8, c: char) -> io::Result<()> {
        self.writer.write_all(&[class])?;
        self.writer.write_all(&(c as u32).to_le_bytes())?;
        self.classes[class as usize] = true;
        Ok(())
    }
}

pub(super) struct Normalizer {
    marks: Vec<(u8, char)>,
    spill: Option<Spill>,
}

impl Normalizer {
    pub(super) fn new() -> io::Result<Self> {
        let mut marks = Vec::new();
        marks
            .try_reserve_exact(MARK_LIMIT)
            .map_err(io::Error::other)?;
        Ok(Self { marks, spill: None })
    }

    pub(super) fn push(&mut self, c: char, vm: &mut Vm<'_>, cancel: &AtomicBool) -> io::Result<()> {
        let mut result = Ok(());
        decompose_canonical(c, |decomposed| {
            if result.is_ok() && !vm.matched {
                result = self.decomposed(decomposed, vm, cancel);
            }
        });
        result
    }

    fn decomposed(&mut self, c: char, vm: &mut Vm<'_>, cancel: &AtomicBool) -> io::Result<()> {
        let class = canonical_combining_class(c);
        // A removed class-zero character (e.g. U+034F) still delimits the
        // canonical ordering segment. Filtering before this would change NFD.
        if class == 0 {
            self.finish(vm, cancel)?;
        }
        if crate::utils::char_is_mark(c) || vm.matched {
            return Ok(());
        }
        if class == 0 {
            return vm.push(c, cancel);
        }
        if let Some(spill) = &mut self.spill {
            return spill.push(class, c);
        }
        if self.marks.len() == MARK_LIMIT {
            let mut spill = Spill {
                writer: BufWriter::new(tempfile::tempfile()?),
                classes: [false; 256],
            };
            for &(class, c) in &self.marks {
                spill.push(class, c)?;
            }
            self.marks.clear();
            spill.push(class, c)?;
            self.spill = Some(spill);
        } else {
            self.marks.push((class, c));
        }
        Ok(())
    }

    pub(super) fn finish(&mut self, vm: &mut Vm<'_>, cancel: &AtomicBool) -> io::Result<()> {
        if let Some(mut spill) = self.spill.take() {
            spill.writer.flush()?;
            // There are only 255 nonzero canonical classes. Stable filtering
            // by each present class reproduces stable NFD ordering with bounded
            // RAM even when an entire file is one combining sequence.
            for class in 1..256 {
                if !spill.classes[class] {
                    continue;
                }
                spill.writer.seek(SeekFrom::Start(0))?;
                let mut reader = BufReader::new(spill.writer.get_mut());
                let mut record = [0; 5];
                let mut work = 0;
                loop {
                    if work % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                        return Err(io::Error::other("Search cancelled"));
                    }
                    if reader.read(&mut record[..1])? == 0 {
                        break;
                    }
                    reader.read_exact(&mut record[1..])?;
                    if record[0] as usize == class {
                        let c = char::from_u32(u32::from_le_bytes(record[1..].try_into().unwrap()))
                            .ok_or_else(|| io::Error::other("Invalid normalization record"))?;
                        vm.push(c, cancel)?;
                        if vm.matched {
                            return Ok(());
                        }
                    }
                    work += 1;
                }
            }
        } else {
            self.marks.sort_by_key(|&(class, _)| class);
            for &(_, c) in &self.marks {
                vm.push(c, cancel)?;
                if vm.matched {
                    break;
                }
            }
            self.marks.clear();
        }
        Ok(())
    }
}
