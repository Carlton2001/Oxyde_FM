//! Worker stream adapters: seek changes a local cursor, never an object's identity.
use super::archive_io_protocol::{invalid, Frame, Message, MAX_BLOCK};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

pub trait Exchange {
    fn exchange(&mut self, frame: Frame) -> io::Result<Frame>;
    fn turbo(&self) -> bool {
        false
    }
}

/// Codecs can retain several readers while sharing one ordered IPC conversation.
pub type Connection<T> = Arc<Mutex<T>>;

fn exchange<T: Exchange>(connection: &Connection<T>, frame: Frame) -> io::Result<Frame> {
    connection.lock().map_err(|_| invalid())?.exchange(frame)
}

fn position(current: u64, length: u64, from: SeekFrom) -> io::Result<u64> {
    match from {
        SeekFrom::Start(offset) => Ok(offset),
        SeekFrom::Current(delta) => current.checked_add_signed(delta).ok_or_else(invalid),
        SeekFrom::End(delta) => length.checked_add_signed(delta).ok_or_else(invalid),
    }
}

pub struct Reader<T> {
    connection: Connection<T>,
    source: u64,
    size: u64,
    cursor: u64,
}

impl<T> Reader<T> {
    pub fn new(connection: Connection<T>, source: u64, size: u64) -> Self {
        Self {
            connection,
            source,
            size,
            cursor: 0,
        }
    }
}

impl<T: Exchange> Read for Reader<T> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let length = output
            .len()
            .min(MAX_BLOCK)
            .min(self.size.saturating_sub(self.cursor).min(usize::MAX as u64) as usize);
        if length == 0 {
            return Ok(0);
        }
        let response = exchange(
            &self.connection,
            Frame::new(Message::Read {
                source: self.source,
                offset: self.cursor,
                length: length as u32,
            }),
        )?;
        if response.message
            != (Message::Bytes {
                source: self.source,
                offset: self.cursor,
            })
            || response.data.is_empty()
            || response.data.len() > length
        {
            return Err(invalid());
        }
        output[..response.data.len()].copy_from_slice(&response.data);
        self.cursor += response.data.len() as u64;
        Ok(response.data.len())
    }
}

impl<T: Exchange> Seek for Reader<T> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let cursor = position(self.cursor, self.size, from)?;
        if cursor > self.size {
            return Err(invalid());
        }
        self.cursor = cursor;
        Ok(cursor)
    }
}

/// Keeps the last fetched window across seeks. `io::BufReader` discards its
/// buffer on every relative seek, which multiplies source requests for codecs
/// that reposition between small reads (ZIP).
pub struct WindowReader<R> {
    inner: R,
    window: Vec<u8>,
    start: u64,
    cursor: u64,
    capacity: usize,
}

impl<R> WindowReader<R> {
    pub fn new(inner: R, capacity: usize) -> Self {
        Self {
            inner,
            window: Vec::new(),
            start: 0,
            cursor: 0,
            capacity: capacity.max(1),
        }
    }
}

impl<R: Read + Seek> Read for WindowReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let end = self.start + self.window.len() as u64;
        if self.cursor < self.start || self.cursor >= end {
            self.inner.seek(SeekFrom::Start(self.cursor))?;
            if output.len() >= self.capacity {
                let count = self.inner.read(output)?;
                self.cursor += count as u64;
                return Ok(count);
            }
            self.window.resize(self.capacity, 0);
            let count = self.inner.read(&mut self.window);
            let count = match count {
                Ok(count) => count,
                Err(error) => {
                    self.window.clear();
                    return Err(error);
                }
            };
            self.window.truncate(count);
            self.start = self.cursor;
            if count == 0 {
                return Ok(0);
            }
        }
        let offset = (self.cursor - self.start) as usize;
        let count = output.len().min(self.window.len() - offset);
        output[..count].copy_from_slice(&self.window[offset..offset + count]);
        self.cursor += count as u64;
        Ok(count)
    }
}

impl<R: Seek> Seek for WindowReader<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.cursor = match from {
            SeekFrom::End(_) => self.inner.seek(from)?,
            _ => position(self.cursor, u64::MAX, from)?,
        };
        Ok(self.cursor)
    }
}

pub struct Writer<T> {
    connection: Connection<T>,
    cursor: u64,
    size: u64,
    limit: u64,
}

impl<T> Writer<T> {
    pub fn new(connection: Connection<T>, limit: u64) -> Self {
        Self {
            connection,
            cursor: 0,
            size: 0,
            limit,
        }
    }
    pub fn size(&self) -> u64 {
        self.size
    }
}

impl<T: Exchange> Write for Writer<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = bytes.len().min(MAX_BLOCK);
        if length == 0 {
            return Ok(0);
        }
        let end = self
            .cursor
            .checked_add(length as u64)
            .filter(|end| *end <= self.limit)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    crate::models::error::ArchiveLimitFailure {
                        kind: crate::models::error::ArchiveLimitKind::EncodedBytes,
                        limit: self.limit,
                    },
                )
            })?;
        let response = exchange(
            &self.connection,
            Frame::data(
                Message::Write {
                    offset: self.cursor,
                },
                bytes[..length].to_vec(),
            ),
        )?;
        if response
            != Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            })
        {
            return Err(invalid());
        }
        self.cursor = end;
        self.size = self.size.max(end);
        Ok(length)
    }
    fn flush(&mut self) -> io::Result<()> {
        if exchange(&self.connection, Frame::new(Message::Flush))?
            != Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            })
        {
            return Err(invalid());
        }
        Ok(())
    }
}

impl<T: Exchange> Seek for Writer<T> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let cursor = position(self.cursor, self.size, from)?;
        if cursor > self.limit {
            return Err(invalid());
        }
        self.cursor = cursor;
        Ok(cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Broker {
        source: Vec<u8>,
        output: Cursor<Vec<u8>>,
        calls: usize,
    }
    impl Exchange for Broker {
        fn exchange(&mut self, frame: Frame) -> io::Result<Frame> {
            self.calls += 1;
            match frame.message {
                Message::Read {
                    source: 9,
                    offset,
                    length,
                } => {
                    let end = offset as usize + length as usize;
                    Ok(Frame::data(
                        Message::Bytes { source: 9, offset },
                        self.source[offset as usize..end].to_vec(),
                    ))
                }
                Message::Write { offset } => {
                    self.output.seek(SeekFrom::Start(offset))?;
                    self.output.write_all(&frame.data)?;
                    Ok(Frame::new(Message::Ack {
                        pause: false,
                        turbo: false,
                    }))
                }
                Message::Flush => Ok(Frame::new(Message::Ack {
                    pause: false,
                    turbo: false,
                })),
                _ => Err(invalid()),
            }
        }
    }
    fn broker() -> Connection<Broker> {
        Arc::new(Mutex::new(Broker {
            source: b"abcdef".to_vec(),
            output: Cursor::new(Vec::new()),
            calls: 0,
        }))
    }

    #[test]
    fn readers_have_independent_positions_and_never_read_a_foreign_resource() {
        let connection = broker();
        let mut first = Reader::new(connection.clone(), 9, 6);
        let mut second = Reader::new(connection.clone(), 9, 6);
        first.seek(SeekFrom::End(-2)).unwrap();
        let mut bytes = [0; 2];
        first.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ef");
        second.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ab");
        assert!(second.seek(SeekFrom::Current(i64::MIN)).is_err());
        assert!(second.seek(SeekFrom::Start(7)).is_err());
        assert_eq!(first.read(&mut bytes).unwrap(), 0);
        assert!(Reader::new(connection, 10, 6).read(&mut bytes).is_err());
    }

    #[test]
    fn output_limit_counts_extents_instead_of_rewritten_bytes() {
        let connection = broker();
        let mut writer = Writer::new(connection.clone(), 6);
        writer.write_all(b"abcdef").unwrap();
        writer.seek(SeekFrom::Start(0)).unwrap();
        writer.write_all(b"ABC").unwrap();
        writer.seek(SeekFrom::End(0)).unwrap();
        assert!(writer.write_all(b"x").is_err());
        assert!(writer.seek(SeekFrom::End(1)).is_err());
        assert_eq!(writer.size(), 6);
        writer.flush().unwrap();
        assert_eq!(connection.lock().unwrap().output.get_ref(), b"ABCdef");
    }

    #[test]
    fn window_reader_serves_backward_and_forward_seeks_without_new_requests() {
        let connection = broker();
        let mut reader = WindowReader::new(Reader::new(connection.clone(), 9, 6), 4);
        let mut bytes = [0; 2];
        reader.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ab");
        reader.seek(SeekFrom::Current(1)).unwrap();
        reader.read_exact(&mut bytes[..1]).unwrap();
        assert_eq!(bytes[0], b'd');
        reader.seek(SeekFrom::Start(0)).unwrap();
        reader.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ab");
        assert_eq!(connection.lock().unwrap().calls, 1);
        reader.seek(SeekFrom::End(-2)).unwrap();
        reader.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ef");
        assert_eq!(reader.read(&mut bytes).unwrap(), 0);
        assert_eq!(connection.lock().unwrap().calls, 2);
        assert!(reader.seek(SeekFrom::Start(7)).is_ok_and(|_| reader.read(&mut bytes).is_err()));
    }

    struct WrongReply;
    impl Exchange for WrongReply {
        fn exchange(&mut self, _: Frame) -> io::Result<Frame> {
            Ok(Frame::data(
                Message::Bytes {
                    source: 9,
                    offset: 1,
                },
                vec![1],
            ))
        }
    }
    #[test]
    fn replies_are_bound_to_the_request_and_failed_io_does_not_move_the_cursor() {
        let connection = Arc::new(Mutex::new(WrongReply));
        let mut reader = Reader::new(connection.clone(), 9, 2);
        assert!(reader.read(&mut [0]).is_err());
        assert_eq!(reader.stream_position().unwrap(), 0);
        let mut writer = Writer::new(connection, 2);
        assert!(writer.write(b"x").is_err());
        assert_eq!(writer.stream_position().unwrap(), 0);
        assert_eq!(writer.size(), 0);
    }
}
