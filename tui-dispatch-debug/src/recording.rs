//! Append-only recordings with bounded frames and cancellation-safe live following.
//!
//! Files start with an eight-byte magic/version header. Each record is UTF-8 JSON
//! prefixed by a little-endian u32 length; zero length marks a clean finish.
//! Readers reject unsupported headers, malformed JSON, and oversized frames.

use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
    time::Duration,
};

use tokio::io::AsyncReadExt;

use crate::{
    observe::DebugRecord,
    records::{RecordRef, SnapshotRecord},
};

const HEADER: &[u8; 8] = b"TUIDBG\x01\n";
/// Maximum encoded bytes in a single record (16 MiB), excluding its length prefix.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
/// EOF polling interval for followers. Filesystem reads run on Tokio's blocking pool.
pub const FOLLOW_INTERVAL: Duration = Duration::from_millis(10);

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

struct Frame(Vec<u8>);
impl Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RECORD_BYTES - self.0.len() {
            return Err(invalid("recording record exceeds frame limit"));
        }
        let needed = self.0.len() + bytes.len();
        if needed > self.0.capacity() {
            let target = needed
                .max(self.0.capacity().saturating_mul(2))
                .min(MAX_RECORD_BYTES);
            self.0.reserve_exact(target - self.0.len());
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A synchronous recording writer; append records in dispatch order.
///
/// Creation refuses existing paths, including symlinks. On Unix the new file has
/// mode 0600. `flush` publishes buffered records to followers; it does not fsync.
/// Call `finish` to publish clean EOF. Drop only flushes buffered bytes and leaves
/// the recording unfinished, permitting recovery after an interrupted session.
/// Disk I/O is explicit and never added to Store dispatch automatically.
pub struct RecordingWriter {
    output: BufWriter<File>,
    frame: Frame,
    failed: bool,
}

impl RecordingWriter {
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(HEADER)?;
        Ok(Self {
            output: BufWriter::new(file),
            frame: Frame(Vec::new()),
            failed: false,
        })
    }

    /// Append a collected dispatch, preserving all IDs, payloads and outcomes.
    pub fn append(&mut self, record: &DebugRecord) -> io::Result<()> {
        self.append_serialized(&RecordRef::from(record))
    }

    /// Append an already owned record, for example when copying a recording.
    pub fn append_record(&mut self, record: &SnapshotRecord) -> io::Result<()> {
        self.append_serialized(record)
    }

    fn append_serialized(&mut self, record: &impl serde::Serialize) -> io::Result<()> {
        self.check_ready()?;
        self.frame.0.clear();
        // Serialize before touching the file: oversize records cannot damage
        // the complete prefix, and the buffer never exceeds MAX_RECORD_BYTES.
        serde_json::to_writer(&mut self.frame, record).map_err(io::Error::other)?;
        self.failed = true;
        self.output
            .write_all(&(self.frame.0.len() as u32).to_le_bytes())?;
        self.output.write_all(&self.frame.0)?;
        self.failed = false;
        Ok(())
    }

    /// Make appended bytes visible to readers. This is not a durability guarantee.
    pub fn flush(&mut self) -> io::Result<()> {
        self.check_ready()?;
        self.failed = true;
        self.output.flush()?;
        self.failed = false;
        Ok(())
    }

    /// Write the finish marker, flush, and close. No further appends are possible.
    pub fn finish(mut self) -> io::Result<()> {
        self.check_ready()?;
        self.output.write_all(&0u32.to_le_bytes())?;
        self.output.flush()
    }

    fn check_ready(&self) -> io::Result<()> {
        if self.failed {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "recording writer cannot continue after an I/O failure",
            ))
        } else {
            Ok(())
        }
    }
}

fn open(path: impl AsRef<Path>) -> io::Result<File> {
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("recording must be a regular file"));
    }
    let mut header = [0; HEADER.len()];
    file.read_exact(&mut header)?;
    if &header != HEADER {
        return Err(invalid("unsupported recording magic or version"));
    }
    Ok(file)
}

enum Progress {
    Read,
    Record(SnapshotRecord),
    Finished,
}

// Only one prefix/payload is retained, including across cancelled next() calls.
struct Decoder {
    bytes: Vec<u8>,
    filled: usize,
    payload: bool,
    finished: bool,
    failed: bool,
}

impl Decoder {
    fn new() -> Self {
        Self {
            bytes: vec![0; 4],
            filled: 0,
            payload: false,
            finished: false,
            failed: false,
        }
    }

    fn advance(&mut self) -> io::Result<Progress> {
        if self.failed {
            return Err(invalid(
                "recording reader cannot continue after invalid data",
            ));
        }
        if self.finished {
            return Ok(Progress::Finished);
        }
        if self.filled < self.bytes.len() {
            return Ok(Progress::Read);
        }
        self.failed = true;
        if !self.payload {
            let length = u32::from_le_bytes(self.bytes[..4].try_into().unwrap()) as usize;
            if length > MAX_RECORD_BYTES {
                return Err(invalid("recording record exceeds frame limit"));
            }
            if length == 0 {
                self.finished = true;
                self.failed = false;
                return Ok(Progress::Finished);
            }
            if length > self.bytes.capacity() {
                self.bytes.reserve_exact(length - self.bytes.len());
            }
            self.bytes.resize(length, 0);
            self.filled = 0;
            self.payload = true;
            self.failed = false;
            return Ok(Progress::Read);
        }
        let record = serde_json::from_slice(&self.bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.bytes.resize(4, 0);
        self.filled = 0;
        self.payload = false;
        self.failed = false;
        Ok(Progress::Record(record))
    }
}

/// Offline reader. Use `read_next` to keep memory independent of file length.
/// Returned records use owned rejection details, shared with socket snapshots.
pub struct RecordingReader {
    input: BufReader<File>,
    decoder: Decoder,
}

/// Complete records recovered from a possibly interrupted recording.
#[derive(Debug)]
pub struct RecordingRecovery {
    pub records: Vec<SnapshotRecord>,
    /// True if EOF precedes the finish marker, even at a record boundary.
    pub incomplete_tail: bool,
}

impl RecordingReader {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            input: BufReader::new(open(path)?),
            decoder: Decoder::new(),
        })
    }

    /// Follow from the first record, waiting at EOF until append or finish.
    ///
    /// Requires a Tokio runtime. Dropped/crashed writers have no finish marker:
    /// cancel `next` to stop waiting and use `recover` for offline inspection.
    /// The opened file is followed; path replacement/rotation is not supported.
    pub fn follow(path: impl AsRef<Path>) -> io::Result<RecordingFollower> {
        Ok(RecordingFollower {
            input: tokio::io::BufReader::new(tokio::fs::File::from_std(open(path)?)),
            decoder: Decoder::new(),
        })
    }

    /// Read one record; unclean EOF is `UnexpectedEof`, not permanent completion.
    pub fn read_next(&mut self) -> io::Result<Option<SnapshotRecord>> {
        loop {
            match self.decoder.advance()? {
                Progress::Record(record) => return Ok(Some(record)),
                Progress::Finished => return Ok(None),
                Progress::Read => {
                    let n = match self
                        .input
                        .read(&mut self.decoder.bytes[self.decoder.filled..])
                    {
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        result => result?,
                    };
                    if n == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "recording has an incomplete tail or no finish marker",
                        ));
                    }
                    self.decoder.filled += n;
                }
            }
        }
    }

    /// Read all remaining records, requiring a finish marker.
    pub fn read_all(mut self) -> io::Result<Vec<SnapshotRecord>> {
        let mut records = Vec::new();
        while let Some(record) = self.read_next()? {
            records.push(record);
        }
        Ok(records)
    }

    /// Retain the complete prefix on truncation. Corrupt frames still error.
    pub fn recover(mut self) -> io::Result<RecordingRecovery> {
        let mut records = Vec::new();
        loop {
            match self.read_next() {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {
                    return Ok(RecordingRecovery {
                        records,
                        incomplete_tail: false,
                    })
                }
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    return Ok(RecordingRecovery {
                        records,
                        incomplete_tail: true,
                    });
                }
                Err(error) => return Err(error),
            }
        }
    }
}

/// Live reader, retaining at most one bounded frame regardless of writer speed.
pub struct RecordingFollower {
    input: tokio::io::BufReader<tokio::fs::File>,
    decoder: Decoder,
}

impl RecordingFollower {
    /// Wait for one record or a finish marker. Cancellation preserves partial
    /// reads: a subsequent call resumes without skipping or duplicating records.
    pub async fn next(&mut self) -> io::Result<Option<SnapshotRecord>> {
        loop {
            match self.decoder.advance()? {
                Progress::Record(record) => return Ok(Some(record)),
                Progress::Finished => return Ok(None),
                Progress::Read => {
                    let n = self
                        .input
                        .read(&mut self.decoder.bytes[self.decoder.filled..])
                        .await?;
                    if n == 0 {
                        tokio::time::sleep(FOLLOW_INTERVAL).await;
                    } else {
                        self.decoder.filled += n;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_storage_is_bounded_even_when_payload_sizes_grow() {
        let mut frame = Frame(Vec::new());
        // A write near the limit must not cause Vec's geometric growth to
        // allocate beyond the frame budget.
        frame
            .write_all(&vec![b'x'; MAX_RECORD_BYTES / 2 + 1])
            .unwrap();
        frame
            .write_all(&vec![b'x'; MAX_RECORD_BYTES / 2 - 1])
            .unwrap();
        assert_eq!(frame.0.len(), MAX_RECORD_BYTES);
        assert!(frame.0.capacity() <= MAX_RECORD_BYTES);
        assert!(frame.write_all(b"x").is_err());

        let mut decoder = Decoder::new();
        for length in [MAX_RECORD_BYTES / 2 + 1, MAX_RECORD_BYTES] {
            decoder.bytes.resize(4, 0);
            decoder.bytes[..4].copy_from_slice(&(length as u32).to_le_bytes());
            decoder.filled = 4;
            decoder.payload = false;
            assert!(matches!(decoder.advance().unwrap(), Progress::Read));
            assert_eq!(decoder.bytes.len(), length);
            assert!(decoder.bytes.capacity() <= MAX_RECORD_BYTES);
        }
        decoder.bytes.resize(4, 0);
        decoder.bytes[..4].copy_from_slice(&((MAX_RECORD_BYTES + 1) as u32).to_le_bytes());
        decoder.filled = 4;
        decoder.payload = false;
        assert!(decoder.advance().is_err());
        assert!(decoder.bytes.capacity() <= MAX_RECORD_BYTES);
    }
}
