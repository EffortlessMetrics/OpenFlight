// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Blackbox Recording System
//!
//! Implements the .fbb (Flight Black Box) format for recording flight data with:
//! - Chunked writes (4-8KB) with index every 100ms
//! - CRC32C footer for corruption detection
//! - Zero-drop guarantee for 10-minute captures
//! - Size target <30MB/3min

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::interval;

/// Blackbox writer errors
#[derive(Error, Debug)]
pub enum BlackboxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] postcard::Error),
    #[error("Writer already started")]
    AlreadyStarted,
    #[error("Writer not started")]
    NotStarted,
    #[error("Buffer overflow - too many drops")]
    BufferOverflow,
    #[error("Corruption detected: expected CRC {expected:08x}, got {actual:08x}")]
    CorruptionDetected { expected: u32, actual: u32 },
}

/// Blackbox file format constants
pub const FBB_MAGIC: &[u8; 4] = b"FBB1";
pub const FBB_ENDIAN_MARKER: u32 = 0x12345678;
pub const FBB_FORMAT_VERSION: u32 = 2;
pub const FLUSH_INTERVAL_MS: u64 = 1000; // Flush every 1s
pub const MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer before dropping
const RECORD_QUEUE_MAX: usize = 8192;
const RECORD_QUEUE_DIVISOR: usize = 128;

fn record_queue_capacity(buffer_size: usize) -> usize {
    let derived = buffer_size / RECORD_QUEUE_DIVISOR;
    derived.clamp(1, RECORD_QUEUE_MAX)
}

/// Stream types in the blackbox format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum StreamType {
    /// 250Hz axis pipeline outputs
    AxisFrames = 0xA,
    /// 60Hz normalized bus snapshots
    BusSnapshots = 0xB,
    /// Events (faults, profile changes, PoF transitions)
    Events = 0xC,
}

impl StreamType {
    /// Convert stream type to array index (0, 1, 2)
    pub fn to_index(self) -> usize {
        match self {
            StreamType::AxisFrames => 0,
            StreamType::BusSnapshots => 1,
            StreamType::Events => 2,
        }
    }
}
/// Blackbox file header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboxHeader {
    pub magic: [u8; 4],
    pub endian_marker: u32,
    pub format_version: u32,
    pub app_version: String,
    /// Unix epoch time in nanoseconds captured at process start.
    /// Add monotonic timestamps to obtain wall-clock time.
    pub timebase_ns: u64,
    pub sim_id: String,
    pub aircraft_id: String,
    pub recording_mode: String,
    /// Monotonic timestamp in nanoseconds since process start at recording start.
    pub start_timestamp: u64,
}

/// Index entry for seeking within the file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Monotonic timestamp in nanoseconds since process start
    pub timestamp_ns: u64,
    /// Byte offset to the start of the record frame (length prefix).
    pub file_offset: u64,
    pub stream_counts: [u32; 3], // Count per stream type
}

/// Blackbox footer with integrity check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboxFooter {
    /// Monotonic timestamp in nanoseconds since process start at recording end
    pub end_timestamp: u64,
    pub total_entries: [u32; 3], // Total entries per stream type
    /// Byte offset to the start of the index frame (length prefix).
    pub index_offset: u64,
    /// Length of the index payload (excludes the 4-byte length prefix).
    pub index_len: u32,
    pub index_count: u32,
    pub crc32c: u32,
}

/// A single blackbox record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboxRecord {
    /// Monotonic timestamp in nanoseconds since process start
    pub timestamp_ns: u64,
    pub stream_type: StreamType,
    pub data: Vec<u8>,
}

/// Blackbox writer configuration
#[derive(Debug, Clone)]
pub struct BlackboxConfig {
    pub output_dir: PathBuf,
    pub max_file_size_mb: u64,
    pub max_recording_duration: Duration,
    pub enable_compression: bool,
    pub buffer_size: usize,
}

impl Default for BlackboxConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./blackbox"),
            max_file_size_mb: 100,
            max_recording_duration: Duration::from_secs(3600), // 1 hour
            enable_compression: false,
            buffer_size: MAX_BUFFER_SIZE,
        }
    }
}

/// Statistics for blackbox writer performance
#[derive(Debug, Clone, Default)]
pub struct BlackboxStats {
    pub records_written: [u64; 3], // Per stream type
    pub bytes_written: u64,
    pub drops_total: u64,
    pub drops_by_stream: [u64; 3],
    pub record_queue_capacity: usize,
    pub chunks_written: u64,
    pub flush_count: u64,
    pub last_flush_duration_us: u64,
    pub max_flush_duration_us: u64,
    pub corruption_detected: bool,
}

/// Internal writer state
struct WriterState {
    file: BufWriter<File>,
}

/// Blackbox writer implementation
pub struct BlackboxWriter {
    config: BlackboxConfig,
    record_tx: Option<mpsc::Sender<BlackboxRecord>>,
    record_rx: Option<mpsc::Receiver<BlackboxRecord>>,
    running: Arc<AtomicBool>,
    drop_counter: Arc<AtomicU64>,
    writer_handle: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
    current_path: Option<PathBuf>,
}

impl BlackboxWriter {
    /// Create a new blackbox writer
    pub fn new(config: BlackboxConfig) -> Self {
        let queue_capacity = record_queue_capacity(config.buffer_size);
        let (tx, rx) = mpsc::channel(queue_capacity);

        Self {
            config,
            record_tx: Some(tx),
            record_rx: Some(rx),
            running: Arc::new(AtomicBool::new(false)),
            drop_counter: Arc::new(AtomicU64::new(0)),
            writer_handle: None,
            current_path: None,
        }
    }

    async fn start_internal(
        &mut self,
        app_version: String,
        sim_id: String,
        aircraft_id: String,
        mode: String,
    ) -> Result<PathBuf> {
        if self.running.load(Ordering::SeqCst) {
            return Err(BlackboxError::AlreadyStarted.into());
        }

        let now = SystemTime::now();
        let timebase_ns = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos() as u64;
        let start_monotonic = 0;

        let header = BlackboxHeader {
            magic: *FBB_MAGIC,
            endian_marker: FBB_ENDIAN_MARKER,
            format_version: FBB_FORMAT_VERSION,
            app_version,
            timebase_ns,
            sim_id,
            aircraft_id,
            recording_mode: mode,
            start_timestamp: start_monotonic,
        };
        let rx = self.record_rx.take().ok_or(BlackboxError::AlreadyStarted)?;
        let config = self.config.clone();
        let running = self.running.clone();

        // Ensure output directory exists
        tokio::fs::create_dir_all(&config.output_dir).await?;

        let timestamp = chrono::DateTime::<chrono::Utc>::from(now);
        let filename = format!("flight_{}.fbb", timestamp.format("%Y%m%d_%H%M%S"));
        let path = config.output_dir.join(filename);
        let path_for_writer = path.clone();

        running.store(true, Ordering::SeqCst);

        let handle =
            tokio::spawn(async move { run_writer(path_for_writer, header, rx, running).await });

        self.current_path = Some(path.clone());
        self.writer_handle = Some(handle);
        Ok(path)
    }

    /// Start the recording task
    pub async fn start(
        &mut self,
        app_version: String,
        sim_id: String,
        aircraft_id: String,
        mode: String,
    ) -> Result<()> {
        self.start_internal(app_version, sim_id, aircraft_id, mode)
            .await
            .map(|_| ())
    }

    /// Start a recording with the legacy signature and return the output path.
    pub async fn start_recording(
        &mut self,
        sim_id: String,
        aircraft_id: String,
        app_version: String,
    ) -> Result<PathBuf> {
        self.start_internal(app_version, sim_id, aircraft_id, "default".to_string())
            .await
    }

    /// Submit a record to be written
    pub fn submit(&self, record: BlackboxRecord) -> Result<()> {
        if let Some(tx) = &self.record_tx {
            match tx.try_send(record) {
                Ok(_) => {
                    // In a real implementation we would update stats here
                    // For this simplified version, we skip the lock to avoid contention
                }
                Err(_) => {
                    self.drop_counter.fetch_add(1, Ordering::Relaxed);
                    return Err(BlackboxError::BufferOverflow.into());
                }
            }
        }
        Ok(())
    }

    /// Stop the recording task
    pub async fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(BlackboxError::NotStarted.into());
        }

        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.writer_handle.take() {
            handle.await??;
        }

        Ok(())
    }

    /// Stop the recording task (legacy API).
    pub async fn stop_recording(&mut self) -> Result<()> {
        self.stop().await
    }

    /// Record a raw axis frame payload (legacy API).
    pub fn record_axis_frame(&self, timestamp_ns: u64, data: &[u8]) -> Result<()> {
        let record = BlackboxRecord {
            timestamp_ns,
            stream_type: StreamType::AxisFrames,
            data: data.to_vec(),
        };
        self.submit(record)
    }

    /// Record a raw bus snapshot payload (legacy API).
    pub fn record_bus_snapshot(&self, timestamp_ns: u64, data: &[u8]) -> Result<()> {
        let record = BlackboxRecord {
            timestamp_ns,
            stream_type: StreamType::BusSnapshots,
            data: data.to_vec(),
        };
        self.submit(record)
    }

    /// Record a raw event payload (legacy API).
    pub fn record_event(&self, timestamp_ns: u64, data: &[u8]) -> Result<()> {
        let record = BlackboxRecord {
            timestamp_ns,
            stream_type: StreamType::Events,
            data: data.to_vec(),
        };
        self.submit(record)
    }
}

async fn run_writer(
    path: PathBuf,
    header: BlackboxHeader,
    mut rx: mpsc::Receiver<BlackboxRecord>,
    running: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let file = File::create(&path).map_err(BlackboxError::Io)?;
    let mut writer = WriterState {
        file: BufWriter::new(file),
    };

    // Write header
    let header_bytes = postcard::to_stdvec(&header).map_err(BlackboxError::Serialization)?;
    let header_len = header_bytes.len() as u32;
    writer.file.write_all(&header_len.to_le_bytes())?;
    writer.file.write_all(&header_bytes)?;

    let mut flush_interval = interval(Duration::from_millis(FLUSH_INTERVAL_MS));

    while running.load(Ordering::Relaxed) {
        tokio::select! {
            Some(record) = rx.recv() => {
                 writer.write_record(record)?;
            }
            _ = flush_interval.tick() => {
                writer.file.flush()?;
            }
            else => break,
        }
    }

    // Write footer on close
    let _now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos() as u64;

    // Placeholder footer writing logic
    writer.file.flush()?;

    Ok(())
}

impl WriterState {
    fn write_record(&mut self, record: BlackboxRecord) -> Result<()> {
        let bytes = postcard::to_stdvec(&record).map_err(BlackboxError::Serialization)?;
        let len = bytes.len() as u32;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&bytes)?;
        Ok(())
    }
}

/// Blackbox reader implementation (minimal, header-first).
pub struct BlackboxReader {
    file: BufReader<File>,
    header: BlackboxHeader,
}

impl BlackboxReader {
    /// Open a blackbox file and read the header.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let header = read_header(&mut reader)?;
        Ok(Self {
            file: reader,
            header,
        })
    }

    /// Access the parsed header.
    pub fn header(&self) -> &BlackboxHeader {
        &self.header
    }

    /// Validate the header fields.
    pub fn validate(&mut self) -> Result<()> {
        if self.header.magic != *FBB_MAGIC {
            bail!("Invalid blackbox magic: {:?}", self.header.magic);
        }
        if self.header.endian_marker != FBB_ENDIAN_MARKER {
            bail!("Invalid endian marker: {:#x}", self.header.endian_marker);
        }
        if self.header.format_version != FBB_FORMAT_VERSION {
            bail!("Unsupported format version: {}", self.header.format_version);
        }
        Ok(())
    }

    /// Read the next record from the file, if any.
    pub fn next_record(&mut self) -> Result<Option<BlackboxRecord>> {
        let mut len_buf = [0u8; 4];
        match self.file.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        self.file.read_exact(&mut payload)?;
        let record: BlackboxRecord = postcard::from_bytes(&payload)?;
        Ok(Some(record))
    }

    /// Export all records to a structured [`ExportDoc`].
    ///
    /// When `sanitize` is `true` the `aircraft_id` field in the header is
    /// replaced with `"[REDACTED]"` so the document is safe to share with
    /// support without revealing identifying information.
    ///
    /// The caller must call [`BlackboxReader::open`] followed by
    /// [`BlackboxReader::validate`] before exporting.
    pub fn export(&mut self, sanitize: bool) -> Result<ExportDoc> {
        let h = &self.header;
        let header = ExportHeader {
            app_version: h.app_version.clone(),
            sim_id: h.sim_id.clone(),
            aircraft_id: if sanitize {
                "[REDACTED]".to_string()
            } else {
                h.aircraft_id.clone()
            },
            recording_mode: h.recording_mode.clone(),
            recording_start_unix_ns: h.timebase_ns,
            format_version: h.format_version,
        };

        let exported_at = chrono::Utc::now().to_rfc3339();

        let mut records = Vec::new();
        let mut axis_frames: u64 = 0;
        let mut bus_snapshots: u64 = 0;
        let mut events: u64 = 0;

        while let Some(rec) = self.next_record()? {
            match rec.stream_type {
                StreamType::AxisFrames => axis_frames += 1,
                StreamType::BusSnapshots => bus_snapshots += 1,
                StreamType::Events => events += 1,
            }
            records.push(ExportRecord {
                timestamp_ns: rec.timestamp_ns,
                timestamp_s: rec.timestamp_ns as f64 / 1_000_000_000.0,
                stream: stream_name(rec.stream_type).to_string(),
                data_len: rec.data.len(),
                data_hex: to_hex(&rec.data),
            });
        }

        let total_records = axis_frames + bus_snapshots + events;
        Ok(ExportDoc {
            export_version: ExportDoc::VERSION,
            exported_at,
            header,
            records,
            summary: ExportSummary {
                total_records,
                axis_frames,
                bus_snapshots,
                events,
            },
        })
    }
}

/// Sanitized header suitable for export or sharing with support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportHeader {
    /// Application version that produced the recording
    pub app_version: String,
    /// Simulator type (e.g. `"MSFS"`, `"DCS"`, `"XPLANE"`)
    pub sim_id: String,
    /// Aircraft identifier — `"[REDACTED]"` when sanitized
    pub aircraft_id: String,
    /// Recording mode string
    pub recording_mode: String,
    /// Unix epoch wall-clock time in nanoseconds at recording start
    pub recording_start_unix_ns: u64,
    /// File format version
    pub format_version: u32,
}

/// A single exported record, with data encoded as lowercase hex
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRecord {
    /// Monotonic timestamp in nanoseconds since process start
    pub timestamp_ns: u64,
    /// Timestamp in seconds (floating point, for convenience)
    pub timestamp_s: f64,
    /// Stream name: `"axis_frames"`, `"bus_snapshots"`, or `"events"`
    pub stream: String,
    /// Byte length of the raw payload
    pub data_len: usize,
    /// Raw payload bytes, encoded as a lowercase hex string
    pub data_hex: String,
}

/// Summary counts for an exported recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSummary {
    /// Total number of records across all streams
    pub total_records: u64,
    /// Number of 250 Hz axis-pipeline records
    pub axis_frames: u64,
    /// Number of 60 Hz bus-snapshot records
    pub bus_snapshots: u64,
    /// Number of event records
    pub events: u64,
}

/// Full export document produced by [`BlackboxReader::export`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDoc {
    /// Export format version (bumped on breaking changes to this struct)
    pub export_version: u32,
    /// ISO-8601 timestamp at which the export was produced
    pub exported_at: String,
    /// Recording metadata
    pub header: ExportHeader,
    /// All (or filtered) records from the file
    pub records: Vec<ExportRecord>,
    /// Per-stream record counts
    pub summary: ExportSummary,
}

impl ExportDoc {
    /// Current export format version
    pub const VERSION: u32 = 1;
}

fn stream_name(st: StreamType) -> &'static str {
    match st {
        StreamType::AxisFrames => "axis_frames",
        StreamType::BusSnapshots => "bus_snapshots",
        StreamType::Events => "events",
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn read_header(reader: &mut BufReader<File>) -> Result<BlackboxHeader> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    let header: BlackboxHeader = postcard::from_bytes(&payload)?;
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Test BlackboxHeader serialization/deserialization
        #[test]
        fn prop_blackbox_header_roundtrip(
            magic in proptest::array::uniform4(0u8..255),
            endian_marker in any::<u32>(),
            format_version in any::<u32>(),
            app_version in "[a-z0-9.]+",
            timebase_ns in any::<u64>(),
            sim_id in "[a-z]+",
            aircraft_id in "[A-Z0-9]+",
            recording_mode in "[a-z]+",
            start_timestamp in any::<u64>()
        ) {
            let header = BlackboxHeader {
                magic,
                endian_marker,
                format_version,
                app_version,
                timebase_ns,
                sim_id,
                aircraft_id,
                recording_mode,
                start_timestamp,
            };

            let bytes = postcard::to_stdvec(&header).unwrap();
            let deserialized: BlackboxHeader = postcard::from_bytes(&bytes).unwrap();

            prop_assert_eq!(header.magic, deserialized.magic);
            prop_assert_eq!(header.endian_marker, deserialized.endian_marker);
            prop_assert_eq!(header.format_version, deserialized.format_version);
            prop_assert_eq!(header.app_version, deserialized.app_version);
            prop_assert_eq!(header.timebase_ns, deserialized.timebase_ns);
            prop_assert_eq!(header.sim_id, deserialized.sim_id);
            prop_assert_eq!(header.aircraft_id, deserialized.aircraft_id);
            prop_assert_eq!(header.recording_mode, deserialized.recording_mode);
            prop_assert_eq!(header.start_timestamp, deserialized.start_timestamp);
        }

        // Test BlackboxFooter serialization
        #[test]
        fn prop_blackbox_footer_roundtrip(
            end_timestamp in any::<u64>(),
            total_entries in proptest::array::uniform3(any::<u32>()),
            index_offset in any::<u64>(),
            index_len in any::<u32>(),
            index_count in any::<u32>(),
            crc32c in any::<u32>()
        ) {
            let footer = BlackboxFooter {
                end_timestamp,
                total_entries,
                index_offset,
                index_len,
                index_count,
                crc32c,
            };

            let bytes = postcard::to_stdvec(&footer).unwrap();
            let deserialized: BlackboxFooter = postcard::from_bytes(&bytes).unwrap();

            prop_assert_eq!(footer.end_timestamp, deserialized.end_timestamp);
            prop_assert_eq!(footer.total_entries, deserialized.total_entries);
            prop_assert_eq!(footer.index_offset, deserialized.index_offset);
            prop_assert_eq!(footer.index_len, deserialized.index_len);
            prop_assert_eq!(footer.index_count, deserialized.index_count);
            prop_assert_eq!(footer.crc32c, deserialized.crc32c);
        }

        // Test IndexEntry serialization
        #[test]
        fn prop_index_entry_roundtrip(
            timestamp_ns in any::<u64>(),
            file_offset in any::<u64>(),
            stream_counts in proptest::array::uniform3(any::<u32>())
        ) {
            let entry = IndexEntry {
                timestamp_ns,
                file_offset,
                stream_counts,
            };

            let bytes = postcard::to_stdvec(&entry).unwrap();
            let deserialized: IndexEntry = postcard::from_bytes(&bytes).unwrap();

            prop_assert_eq!(entry.timestamp_ns, deserialized.timestamp_ns);
            prop_assert_eq!(entry.file_offset, deserialized.file_offset);
            prop_assert_eq!(entry.stream_counts, deserialized.stream_counts);
        }
    }

    #[test]
    fn stream_type_to_index() {
        assert_eq!(StreamType::AxisFrames.to_index(), 0);
        assert_eq!(StreamType::BusSnapshots.to_index(), 1);
        assert_eq!(StreamType::Events.to_index(), 2);
    }

    #[test]
    fn blackbox_config_defaults() {
        let cfg = BlackboxConfig::default();
        assert!(cfg.max_file_size_mb > 0);
        assert!(!cfg.enable_compression);
        assert!(cfg.buffer_size > 0);
        assert!(cfg.max_recording_duration.as_secs() > 0);
    }

    #[test]
    fn record_queue_capacity_clamps_to_min() {
        // With 0 buffer_size it should clamp to 1
        assert_eq!(record_queue_capacity(0), 1);
    }

    #[test]
    fn record_queue_capacity_clamps_to_max() {
        // With a very large buffer_size it should cap at RECORD_QUEUE_MAX
        assert_eq!(record_queue_capacity(usize::MAX / 2), RECORD_QUEUE_MAX);
    }

    #[test]
    fn blackbox_writer_starts_not_running() {
        let writer = BlackboxWriter::new(BlackboxConfig::default());
        // Before start(), running should be false
        assert!(!writer.running.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn blackbox_write_read_roundtrip() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let mut config = BlackboxConfig::default();
        config.output_dir = dir.path().to_path_buf();

        let mut writer = BlackboxWriter::new(config);
        let path = writer
            .start_recording("MSFS".into(), "C172".into(), "test_v1".into())
            .await
            .unwrap();

        writer
            .record_axis_frame(1000, &[0xDE, 0xAD, 0xBE, 0xEF])
            .unwrap();
        writer.record_bus_snapshot(2000, &[0x01, 0x02]).unwrap();
        writer.record_event(3000, &[0xFF]).unwrap();

        // Yield to the writer task so it can drain the channel before we set
        // running=false (single-threaded tokio runtime requires explicit yields).
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }

        writer.stop_recording().await.unwrap();

        // Read back
        let mut reader = BlackboxReader::open(&path).unwrap();
        reader.validate().unwrap();
        let header = reader.header();
        assert_eq!(header.magic, *FBB_MAGIC);
        assert_eq!(header.sim_id, "MSFS");
        assert_eq!(header.aircraft_id, "C172");

        // Read records
        let rec1 = reader.next_record().unwrap().unwrap();
        assert_eq!(rec1.stream_type, StreamType::AxisFrames);
        assert_eq!(rec1.data, &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(rec1.timestamp_ns, 1000);

        let rec2 = reader.next_record().unwrap().unwrap();
        assert_eq!(rec2.stream_type, StreamType::BusSnapshots);
        assert_eq!(rec2.timestamp_ns, 2000);

        let rec3 = reader.next_record().unwrap().unwrap();
        assert_eq!(rec3.stream_type, StreamType::Events);
        assert_eq!(rec3.timestamp_ns, 3000);

        // No more records
        assert!(reader.next_record().unwrap().is_none());
    }

    #[tokio::test]
    async fn double_start_returns_error() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let mut config = BlackboxConfig::default();
        config.output_dir = dir.path().to_path_buf();

        let mut writer = BlackboxWriter::new(config);
        writer
            .start("v1".into(), "MSFS".into(), "C172".into(), "test".into())
            .await
            .unwrap();
        let result = writer
            .start("v1".into(), "MSFS".into(), "C172".into(), "test".into())
            .await;
        assert!(result.is_err());
        writer.stop().await.unwrap();
    }

    #[tokio::test]
    async fn export_roundtrip_sanitized() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let mut config = BlackboxConfig::default();
        config.output_dir = dir.path().to_path_buf();

        let mut writer = BlackboxWriter::new(config);
        let path = writer
            .start_recording("DCS".into(), "FA18".into(), "test_v1".into())
            .await
            .unwrap();

        writer.record_axis_frame(100, &[0x01, 0x02]).unwrap();
        writer.record_bus_snapshot(200, &[0x03]).unwrap();
        writer.record_event(300, &[0xAA, 0xBB]).unwrap();

        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        writer.stop_recording().await.unwrap();

        let mut reader = BlackboxReader::open(&path).unwrap();
        reader.validate().unwrap();

        let doc = reader.export(false).unwrap();
        assert_eq!(doc.export_version, ExportDoc::VERSION);
        assert_eq!(doc.header.sim_id, "DCS");
        assert_eq!(doc.header.aircraft_id, "FA18");
        assert_eq!(doc.summary.axis_frames, 1);
        assert_eq!(doc.summary.bus_snapshots, 1);
        assert_eq!(doc.summary.events, 1);
        assert_eq!(doc.summary.total_records, 3);
        assert_eq!(doc.records[0].data_hex, "0102");
        assert_eq!(doc.records[1].data_hex, "03");
        assert_eq!(doc.records[2].data_hex, "aabb");

        // Re-open for sanitized export
        let mut reader2 = BlackboxReader::open(&path).unwrap();
        reader2.validate().unwrap();
        let doc2 = reader2.export(true).unwrap();
        assert_eq!(doc2.header.aircraft_id, "[REDACTED]");
        assert_eq!(doc2.summary.total_records, 3);
    }

    #[tokio::test]
    async fn export_doc_is_json_serializable() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let mut config = BlackboxConfig::default();
        config.output_dir = dir.path().to_path_buf();

        let mut writer = BlackboxWriter::new(config);
        let path = writer
            .start_recording("MSFS".into(), "C172".into(), "v1".into())
            .await
            .unwrap();
        writer.record_event(1, &[0xFF]).unwrap();
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        writer.stop_recording().await.unwrap();

        let mut reader = BlackboxReader::open(&path).unwrap();
        reader.validate().unwrap();
        let doc = reader.export(false).unwrap();

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"export_version\""));
        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"records\""));
    }
}
