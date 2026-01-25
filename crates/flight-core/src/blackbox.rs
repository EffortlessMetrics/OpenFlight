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
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::time;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

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
pub const CHUNK_SIZE: usize = 6 * 1024; // 6KB chunks
pub const INDEX_INTERVAL_MS: u64 = 100; // Index every 100ms
pub const FLUSH_INTERVAL_MS: u64 = 1000; // Flush every 1s
pub const MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer before dropping
const FOOTER_LEN_BYTES: u64 = 4;
const MAX_HEADER_LEN: usize = 1 << 20; // 1MB
const MAX_FOOTER_LEN: usize = 1 << 20; // 1MB
const MAX_INDEX_LEN: usize = 64 * 1024 * 1024; // 64MB
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
    pub total_entries: [u64; 3], // Total entries per stream type
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
    file_path: PathBuf,
    file_offset: u64,
    index_entries: Vec<IndexEntry>,
    last_index_timestamp_ns: Option<u64>,
    stream_counters: [u32; 3],
    last_record_timestamp_ns: u64,
    record_buffer: Vec<u8>,
}

/// Blackbox writer implementation
pub struct BlackboxWriter {
    config: BlackboxConfig,
    record_tx: Option<mpsc::Sender<BlackboxRecord>>,
    record_rx: Option<mpsc::Receiver<BlackboxRecord>>,
    running: Arc<AtomicBool>,
    stats: Arc<Mutex<BlackboxStats>>,
    drop_counter: Arc<AtomicU64>,
    current_header: Option<BlackboxHeader>,
    writer_handle: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

impl BlackboxWriter {
    /// Create a new blackbox writer
    pub fn new(config: BlackboxConfig) -> Self {
        let queue_capacity = record_queue_capacity(config.buffer_size);
        let (record_tx, record_rx) = mpsc::channel(queue_capacity);
        let stats = Arc::new(Mutex::new(BlackboxStats {
            record_queue_capacity: queue_capacity,
            ..Default::default()
        }));

        Self {
            config,
            record_tx: Some(record_tx),
            record_rx: Some(record_rx),
            running: Arc::new(AtomicBool::new(false)),
            stats,
            drop_counter: Arc::new(AtomicU64::new(0)),
            current_header: None,
            writer_handle: None,
        }
    }

    /// Start recording to a new blackbox file
    pub async fn start_recording(
        &mut self,
        sim_id: String,
        aircraft_id: String,
        app_version: String,
    ) -> Result<PathBuf> {
        if self.running.load(Ordering::Acquire) {
            return Err(BlackboxError::AlreadyStarted.into());
        }

        // Create output directory
        std::fs::create_dir_all(&self.config.output_dir)
            .context("Failed to create blackbox output directory")?;

        // Generate filename with timestamp
        let wall_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!(
            "flight_{}_{}_{}_{}.fbb",
            sim_id,
            aircraft_id,
            app_version.replace('.', "_"),
            wall_timestamp
        );
        let filepath = self.config.output_dir.join(filename);

        // Create and store header for later retrieval
        let header = BlackboxHeader {
            magic: *FBB_MAGIC,
            endian_marker: FBB_ENDIAN_MARKER,
            format_version: FBB_FORMAT_VERSION,
            app_version,
            timebase_ns: time::unix_base_ns(),
            sim_id,
            aircraft_id,
            recording_mode: "normal".to_string(),
            start_timestamp: time::monotonic_now_ns(),
        };

        self.current_header = Some(header.clone());

        let file = File::create(&filepath).context("Failed to create blackbox file")?;
        let mut writer = BufWriter::new(file);
        let header_bytes = wire::encode_alloc(&header).map_err(BlackboxError::from)?;
        wire::write_len_prefixed(&mut writer, &header_bytes)?;
        let file_offset = 4_u64 + header_bytes.len() as u64;

        self.running.store(true, Ordering::Release);

        // Start background writer task
        let record_rx = self.record_rx.take().unwrap();
        let running = Arc::clone(&self.running);
        let stats = Arc::clone(&self.stats);
        let drop_counter = Arc::clone(&self.drop_counter);
        let config = self.config.clone();
        let writer_path = filepath.clone();
        let state = WriterState {
            file: writer,
            file_path: writer_path,
            file_offset,
            index_entries: Vec::new(),
            last_index_timestamp_ns: None,
            stream_counters: [0; 3],
            last_record_timestamp_ns: 0,
            record_buffer: vec![0u8; config.buffer_size.max(1)],
        };

        self.writer_handle = Some(tokio::spawn(async move {
            Self::writer_task(record_rx, running, stats, drop_counter, config, state).await
        }));

        info!("Blackbox recording started: {}", filepath.display());
        Ok(filepath)
    }

    /// Stop recording and finalize the file
    pub async fn stop_recording(&mut self) -> Result<BlackboxStats> {
        if !self.running.load(Ordering::Acquire) {
            return Err(BlackboxError::NotStarted.into());
        }

        self.running.store(false, Ordering::Release);
        self.record_tx.take(); // Close channel to drain writer task

        if let Some(handle) = self.writer_handle.take() {
            match handle.await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => return Err(err),
                Err(err) => {
                    return Err(anyhow::anyhow!("Blackbox writer task failed: {err}"));
                }
            }
        }

        // Get final stats from the shared stats
        let final_stats = self.stats.lock().unwrap().clone();

        info!("Blackbox recording stopped. Stats: {:?}", final_stats);
        Ok(final_stats)
    }

    /// Record an axis frame (Stream A - 250Hz)
    pub fn record_axis_frame(&self, timestamp_ns: u64, data: &[u8]) -> Result<()> {
        self.record_data(timestamp_ns, StreamType::AxisFrames, data)
    }

    /// Record a bus snapshot (Stream B - 60Hz)
    pub fn record_bus_snapshot(&self, timestamp_ns: u64, data: &[u8]) -> Result<()> {
        self.record_data(timestamp_ns, StreamType::BusSnapshots, data)
    }

    /// Record an event (Stream C - variable rate)
    pub fn record_event(&self, timestamp_ns: u64, data: &[u8]) -> Result<()> {
        self.record_data(timestamp_ns, StreamType::Events, data)
    }

    /// Get current recording statistics
    pub fn get_stats(&self) -> BlackboxStats {
        self.stats.lock().unwrap().clone()
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Internal method to record data
    fn record_data(&self, timestamp_ns: u64, stream_type: StreamType, data: &[u8]) -> Result<()> {
        if !self.running.load(Ordering::Acquire) {
            return Ok(()); // Silently ignore if not recording
        }

        let record = BlackboxRecord {
            timestamp_ns,
            stream_type,
            data: data.to_vec(),
        };

        if let Some(ref tx) = self.record_tx {
            match tx.try_send(record) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_record)) => {
                    Self::note_drop(
                        &self.drop_counter,
                        &self.stats,
                        stream_type,
                        "queue full",
                    );
                }
                Err(mpsc::error::TrySendError::Closed(_record)) => {
                    Self::note_drop(
                        &self.drop_counter,
                        &self.stats,
                        stream_type,
                        "channel closed",
                    );
                }
            }
        }

        Ok(())
    }

    fn note_drop(
        drop_counter: &AtomicU64,
        stats: &Arc<Mutex<BlackboxStats>>,
        stream_type: StreamType,
        reason: &'static str,
    ) {
        let previous = drop_counter.fetch_add(1, Ordering::Relaxed);
        {
            let mut stats_guard = stats.lock().unwrap();
            stats_guard.drops_total += 1;
            stats_guard.drops_by_stream[stream_type.to_index()] += 1;
        }
        if previous == 0 {
            warn!(
                target: "blackbox",
                "Blackbox drops detected (reason={}, stream={:?})",
                reason,
                stream_type
            );
        }
    }

    /// Background writer task
    async fn writer_task(
        mut record_rx: mpsc::Receiver<BlackboxRecord>,
        running: Arc<AtomicBool>,
        stats: Arc<Mutex<BlackboxStats>>,
        drop_counter: Arc<AtomicU64>,
        config: BlackboxConfig,
        mut state: WriterState,
    ) -> Result<()> {
        let mut flush_interval = interval(Duration::from_millis(FLUSH_INTERVAL_MS));
        let index_interval_ns = INDEX_INTERVAL_MS.saturating_mul(1_000_000);

        loop {
            tokio::select! {
                // Receive records
                record = record_rx.recv() => {
                    match record {
                        Some(record) => {
                            let record_size = record.data.len() + 32; // Estimate overhead

                            // Simple drop logic: if record is too large for buffer, drop it
                            if record_size > config.buffer_size / 10 { // Drop if record > 10% of buffer
                                Self::note_drop(
                                    &drop_counter,
                                    &stats,
                                    record.stream_type,
                                    "record too large",
                                );
                            } else {
                                let stream_index = record.stream_type.to_index();
                                state.stream_counters[stream_index] =
                                    state.stream_counters[stream_index].saturating_add(1);

                                {
                                    let mut stats_guard = stats.lock().unwrap();
                                    stats_guard.records_written[stream_index] += 1;
                                    stats_guard.bytes_written += record.data.len() as u64;
                                }

                                state.last_record_timestamp_ns = record.timestamp_ns;

                                let record_offset = state.file_offset;
                                let mut record_bytes_storage = Vec::new();
                                let record_bytes = match wire::encode_into_slice(
                                    &record,
                                    &mut state.record_buffer,
                                ) {
                                    Ok(len) => &state.record_buffer[..len],
                                    Err(postcard::Error::SerializeBufferFull) => {
                                        record_bytes_storage = wire::encode_alloc(&record)?;
                                        record_bytes_storage.as_slice()
                                    }
                                    Err(err) => return Err(err.into()),
                                };

                                wire::write_len_prefixed(&mut state.file, record_bytes)?;
                                state.file_offset += 4_u64 + record_bytes.len() as u64;

                                let should_index = state.last_index_timestamp_ns.map_or(true, |last| {
                                    record.timestamp_ns.saturating_sub(last) >= index_interval_ns
                                });
                                if should_index {
                                    state.index_entries.push(IndexEntry {
                                        timestamp_ns: record.timestamp_ns,
                                        file_offset: record_offset,
                                        stream_counts: state.stream_counters,
                                    });
                                    state.last_index_timestamp_ns = Some(record.timestamp_ns);
                                }
                            }
                        }
                        None => break, // Channel closed
                    }
                }

                // Periodic flush
                _ = flush_interval.tick() => {
                    let flush_start = Instant::now();
                    state.file.flush()?;
                    let flush_duration = flush_start.elapsed().as_micros() as u64;

                    let mut stats_guard = stats.lock().unwrap();
                    stats_guard.flush_count += 1;
                    stats_guard.last_flush_duration_us = flush_duration;
                    stats_guard.max_flush_duration_us =
                        stats_guard.max_flush_duration_us.max(flush_duration);
                }
            }
        }

        if !running.load(Ordering::Acquire) {
            state.file.flush()?;
        }

        Self::finalize_writer(&mut state, &stats)?;
        debug!("Blackbox writer task finished");
        Ok(())
    }

    fn finalize_writer(state: &mut WriterState, stats: &Arc<Mutex<BlackboxStats>>) -> Result<()> {
        let index_offset = state.file_offset;
        let index_bytes = wire::encode_alloc(&state.index_entries)?;
        let index_len = u32::try_from(index_bytes.len())
            .map_err(|_| anyhow::anyhow!("Index length exceeds u32::MAX"))?;
        let index_count = u32::try_from(state.index_entries.len())
            .map_err(|_| anyhow::anyhow!("Index entry count exceeds u32::MAX"))?;

        wire::write_len_prefixed(&mut state.file, &index_bytes)?;
        state.file_offset += 4_u64 + index_bytes.len() as u64;
        state.file.flush()?;

        let crc32c = {
            let mut file =
                File::open(&state.file_path).context("Failed to open blackbox file for CRC")?;
            calculate_crc32c_len(&mut file, state.file_offset)?
        };

        let total_entries = {
            let stats_guard = stats.lock().unwrap();
            stats_guard.records_written
        };

        let footer = BlackboxFooter {
            end_timestamp: state.last_record_timestamp_ns,
            total_entries,
            index_offset,
            index_len,
            index_count,
            crc32c,
        };

        let footer_bytes = wire::encode_alloc(&footer)?;
        let footer_len = u32::try_from(footer_bytes.len())
            .map_err(|_| anyhow::anyhow!("Footer length exceeds u32::MAX"))?;
        state.file.write_all(&footer_bytes)?;
        state.file.write_all(&footer_len.to_le_bytes())?;
        state.file.flush()?;

        Ok(())
    }
}

/// Blackbox reader for validation and replay
pub struct BlackboxReader {
    file: File,
    header: BlackboxHeader,
    footer: BlackboxFooter,
    index: Vec<IndexEntry>,
    data_len: u64,
}

impl BlackboxReader {
    /// Open and validate a blackbox file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path.as_ref()).context("Failed to open blackbox file")?;

        let header_bytes = wire::read_len_prefixed(&mut file, MAX_HEADER_LEN)?;
        let header: BlackboxHeader = wire::decode(&header_bytes).map_err(BlackboxError::from)?;

        if header.magic != *FBB_MAGIC {
            return Err(anyhow::anyhow!("Invalid blackbox magic"));
        }
        if header.endian_marker != FBB_ENDIAN_MARKER {
            return Err(anyhow::anyhow!("Unsupported blackbox endianness"));
        }
        if header.format_version != FBB_FORMAT_VERSION {
            warn!(
                "Blackbox format version mismatch: file={}, expected={}",
                header.format_version, FBB_FORMAT_VERSION
            );
        }

        let file_len = file.metadata()?.len();
        if file_len < FOOTER_LEN_BYTES {
            return Err(anyhow::anyhow!("Blackbox file too small"));
        }

        file.seek(SeekFrom::End(-(FOOTER_LEN_BYTES as i64)))?;
        let mut footer_len_bytes = [0u8; 4];
        file.read_exact(&mut footer_len_bytes)?;
        let footer_len = u32::from_le_bytes(footer_len_bytes) as u64;
        if footer_len > MAX_FOOTER_LEN as u64 {
            return Err(anyhow::anyhow!("Footer length too large"));
        }
        let footer_len_usize =
            usize::try_from(footer_len).map_err(|_| anyhow::anyhow!("Footer length too large"))?;

        let footer_start = file_len
            .checked_sub(FOOTER_LEN_BYTES + footer_len)
            .ok_or_else(|| anyhow::anyhow!("Blackbox footer length exceeds file size"))?;
        file.seek(SeekFrom::Start(footer_start))?;

        let mut footer_bytes = vec![0u8; footer_len_usize];
        file.read_exact(&mut footer_bytes)?;
        let footer: BlackboxFooter = wire::decode(&footer_bytes).map_err(BlackboxError::from)?;

        let data_len = footer_start;
        if footer.index_offset > data_len {
            return Err(anyhow::anyhow!("Index offset exceeds data length"));
        }
        if footer.index_len > 0 {
            if footer.index_len as usize > MAX_INDEX_LEN {
                return Err(anyhow::anyhow!("Index length too large"));
            }
            let index_end = footer
                .index_offset
                .checked_add(4_u64 + footer.index_len as u64)
                .ok_or_else(|| anyhow::anyhow!("Index range overflows"))?;
            if index_end > data_len {
                return Err(anyhow::anyhow!("Index range exceeds data length"));
            }
        }

        let crc32c = calculate_crc32c_len(&mut file, data_len)?;
        if crc32c != footer.crc32c {
            return Err(BlackboxError::CorruptionDetected {
                expected: footer.crc32c,
                actual: crc32c,
            }
            .into());
        }

        let mut index = Vec::new();
        if footer.index_len > 0 {
            file.seek(SeekFrom::Start(footer.index_offset))?;
            let index_bytes = wire::read_len_prefixed(&mut file, footer.index_len as usize)?;
            if footer.index_len != index_bytes.len() as u32 {
                warn!(
                    "Index length mismatch: footer={}, actual={}",
                    footer.index_len,
                    index_bytes.len()
                );
            }
            index = wire::decode(&index_bytes).map_err(BlackboxError::from)?;
        }

        Ok(Self {
            file,
            header,
            footer,
            index,
            data_len,
        })
    }

    /// Get file header information
    pub fn header(&self) -> &BlackboxHeader {
        &self.header
    }

    /// Get file footer information  
    pub fn footer(&self) -> &BlackboxFooter {
        &self.footer
    }

    /// Get index entries
    pub fn index(&self) -> &[IndexEntry] {
        &self.index
    }

    /// Validate file integrity
    pub fn validate(&mut self) -> Result<bool> {
        let crc32c = calculate_crc32c_len(&mut self.file, self.data_len)?;
        if crc32c != self.footer.crc32c {
            return Err(BlackboxError::CorruptionDetected {
                expected: self.footer.crc32c,
                actual: crc32c,
            }
            .into());
        }

        // Additional validation could include:
        // - Index consistency
        // - Record count verification
        // - Timestamp monotonicity

        info!("Blackbox file validation passed");
        Ok(true)
    }
}

fn calculate_crc32c_len(file: &mut File, len: u64) -> Result<u32> {
    let mut hasher = crc32c::Crc32cHasher::new();
    let mut buffer = [0u8; 8192];
    file.seek(SeekFrom::Start(0))?;

    let mut remaining = len;
    while remaining > 0 {
        let to_read = buffer.len().min(remaining as usize);
        let bytes_read = file.read(&mut buffer[..to_read])?;
        if bytes_read == 0 {
            return Err(anyhow::anyhow!("Unexpected EOF while computing CRC32C"));
        }
        hasher.update(&buffer[..bytes_read]);
        remaining = remaining.saturating_sub(bytes_read as u64);
    }

    Ok(hasher.finalize())
}

mod wire {
    use super::io;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::io::{Read, Write};

    pub fn write_len_prefixed<W: Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
        let len: u32 = bytes
            .len()
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "length exceeds u32"))?;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(bytes)?;
        Ok(())
    }

    pub fn read_len_prefixed<R: Read>(reader: &mut R, max_len: usize) -> io::Result<Vec<u8>> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        if len > max_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame too large",
            ));
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn encode_alloc<T: Serialize>(value: &T) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(value)
    }

    pub fn encode_into_slice<T: Serialize>(
        value: &T,
        buffer: &mut [u8],
    ) -> Result<usize, postcard::Error> {
        let used = postcard::to_slice(value, buffer)?;
        Ok(used.len())
    }

    pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

// Add CRC32C hasher (simplified implementation)
mod crc32c {
    #[allow(dead_code)]
    pub struct Crc32cHasher {
        state: u32,
    }

    #[allow(dead_code)]
    impl Crc32cHasher {
        pub fn new() -> Self {
            Self { state: 0xFFFFFFFF }
        }

        pub fn update(&mut self, data: &[u8]) {
            // Simplified CRC32C implementation
            // In production, use a proper CRC32C library like `crc32c` crate
            for &byte in data {
                self.state =
                    (self.state >> 8) ^ CRC32C_TABLE[((self.state ^ byte as u32) & 0xFF) as usize];
            }
        }

        pub fn finalize(self) -> u32 {
            self.state ^ 0xFFFFFFFF
        }
    }

    // Simplified CRC32C table (first 16 entries for demo)
    #[allow(dead_code)]
    const CRC32C_TABLE: [u32; 256] = [
        0x00000000, 0xF26B8303, 0xE13B70F7, 0x1350F3F4, 0xC79A971F, 0x35F1141C, 0x26A1E7E8,
        0xD4CA64EB, 0x8AD958CF, 0x78B2DBCC, 0x6BE22838, 0x9989AB3B, 0x4D43CFD0, 0xBF284CD3,
        0xAC78BF27, 0x5E133C24, 0x105EC76F, 0xE235446C, 0xF165B798, 0x030E349B, 0xD7C45070,
        0x25AFD373, 0x36FF2087, 0xC494A384, 0x9A879FA0, 0x68EC1CA3, 0x7BBCEF57, 0x89D76C54,
        0x5D1D08BF, 0xAF768BBC, 0xBC267848, 0x4E4DFB4B, 0x20BD8EDE, 0xD2D60DDD, 0xC186FE29,
        0x33ED7D2A, 0xE72719C1, 0x154C9AC2, 0x061C6936, 0xF477EA35, 0xAA64D611, 0x580F5512,
        0x4B5FA6E6, 0xB93425E5, 0x6DFE410E, 0x9F95C20D, 0x8CC531F9, 0x7EAEB2FA, 0x30E349B1,
        0xC288CAB2, 0xD1D83946, 0x23B3BA45, 0xF779DEAE, 0x05125DAD, 0x1642AE59, 0xE4292D5A,
        0xBA3A117E, 0x4851927D, 0x5B016189, 0xA96AE28A, 0x7DA08661, 0x8FCB0562, 0x9C9BF696,
        0x6EF07595, 0x417B1DBC, 0xB3109EBF, 0xA0406D4B, 0x522BEE48, 0x86E18AA3, 0x748A09A0,
        0x67DAFA54, 0x95B17957, 0xCBA24573, 0x39C9C670, 0x2A993584, 0xD8F2B687, 0x0C38D26C,
        0xFE53516F, 0xED03A29B, 0x1F682198, 0x5125DAD3, 0xA34E59D0, 0xB01EAA24, 0x42752927,
        0x96BF4DCC, 0x64D4CECF, 0x77843D3B, 0x85EFBE38, 0xDBFC821C, 0x2997011F, 0x3AC7F2EB,
        0xC8AC71E8, 0x1C661503, 0xEE0D9600, 0xFD5D65F4, 0x0F36E6F7, 0x61C69362, 0x93AD1061,
        0x80FDE395, 0x72966096, 0xA65C047D, 0x5437877E, 0x4767748A, 0xB50CF789, 0xEB1FCBAD,
        0x197448AE, 0x0A24BB5A, 0xF84F3859, 0x2C855CB2, 0xDEEEDFB1, 0xCDBE2C45, 0x3FD5AF46,
        0x7198540D, 0x83F3D70E, 0x90A324FA, 0x62C8A7F9, 0xB602C312, 0x44694011, 0x5739B3E5,
        0xA55230E6, 0xFB410CC2, 0x092A8FC1, 0x1A7A7C35, 0xE811FF36, 0x3CDB9BDD, 0xCEB018DE,
        0xDDE0EB2A, 0x2F8B6829, 0x82F63B78, 0x709DB87B, 0x63CD4B8F, 0x91A6C88C, 0x456CAC67,
        0xB7072F64, 0xA457DC90, 0x563C5F93, 0x082F63B7, 0xFA44E0B4, 0xE9141340, 0x1B7F9043,
        0xCFB5F4A8, 0x3DDE77AB, 0x2E8E845F, 0xDCE5075C, 0x92A8FC17, 0x60C37F14, 0x73938CE0,
        0x81F80FE3, 0x55326B08, 0xA759E80B, 0xB4091BFF, 0x466298FC, 0x1871A4D8, 0xEA1A27DB,
        0xF94AD42F, 0x0B21572C, 0xDFEB33C7, 0x2D80B0C4, 0x3ED04330, 0xCCBBC033, 0xA24BB5A6,
        0x502036A5, 0x4370C551, 0xB11B4652, 0x65D122B9, 0x97BAA1BA, 0x84EA524E, 0x7681D14D,
        0x2892ED69, 0xDAF96E6A, 0xC9A99D9E, 0x3BC21E9D, 0xEF087A76, 0x1D63F975, 0x0E330A81,
        0xFC588982, 0xB21572C9, 0x407EF1CA, 0x532E023E, 0xA145813D, 0x758FE5D6, 0x87E466D5,
        0x94B49521, 0x66DF1622, 0x38CC2A06, 0xCAA7A905, 0xD9F75AF1, 0x2B9CD9F2, 0xFF56BD19,
        0x0D3D3E1A, 0x1E6DCDEE, 0xEC064EED, 0xC38D26C4, 0x31E6A5C7, 0x22B65633, 0xD0DDD530,
        0x0417B1DB, 0xF67C32D8, 0xE52CC12C, 0x1747422F, 0x49547E0B, 0xBB3FFD08, 0xA86F0EFC,
        0x5A048DFF, 0x8ECEE914, 0x7CA56A17, 0x6FF599E3, 0x9D9E1AE0, 0xD3D3E1AB, 0x21B862A8,
        0x32E8915C, 0xC083125F, 0x144976B4, 0xE622F5B7, 0xF5720643, 0x07198540, 0x590AB964,
        0xAB613A67, 0xB831C993, 0x4A5A4A90, 0x9E902E7B, 0x6CFBAD78, 0x7FAB5E8C, 0x8DC0DD8F,
        0xE330A81A, 0x115B2B19, 0x020BD8ED, 0xF0605BEE, 0x24AA3F05, 0xD6C1BC06, 0xC5914FF2,
        0x37FACCF1, 0x69E9F0D5, 0x9B8273D6, 0x88D28022, 0x7AB90321, 0xAE7367CA, 0x5C18E4C9,
        0x4F48173D, 0xBD23943E, 0xF36E6F75, 0x0105EC76, 0x12551F82, 0xE03E9C81, 0x34F4F86A,
        0xC69F7B69, 0xD5CF889D, 0x27A40B9E, 0x79B737BA, 0x8BDCB4B9, 0x988C474D, 0x6AE7C44E,
        0xBE2DA0A5, 0x4C4623A6, 0x5F16D052, 0xAD7D5351,
    ];
}

#[cfg(test)]

mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_blackbox_basic_recording() {
        let temp_dir = TempDir::new().unwrap();
        let config = BlackboxConfig {
            output_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut writer = BlackboxWriter::new(config);

        // Start recording
        let filepath = writer
            .start_recording(
                "test_sim".to_string(),
                "test_aircraft".to_string(),
                "1.0.0".to_string(),
            )
            .await
            .unwrap();

        // Record some data
        writer.record_axis_frame(1000, b"axis_data_1").unwrap();
        writer.record_bus_snapshot(2000, b"bus_data_1").unwrap();
        writer.record_event(3000, b"event_data_1").unwrap();

        // Wait a bit for async processing
        sleep(Duration::from_millis(50)).await;

        // Stop recording
        let stats = writer.stop_recording().await.unwrap();

        // Verify file exists
        assert!(filepath.exists());

        // Verify stats
        assert_eq!(stats.records_written[StreamType::AxisFrames.to_index()], 1);
        assert_eq!(
            stats.records_written[StreamType::BusSnapshots.to_index()],
            1
        );
        assert_eq!(stats.records_written[StreamType::Events.to_index()], 1);
        assert_eq!(stats.drops_total, 0);
    }

    #[tokio::test]
    async fn test_blackbox_file_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config = BlackboxConfig {
            output_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut writer = BlackboxWriter::new(config);

        // Record and finalize a file
        let filepath = writer
            .start_recording(
                "test_sim".to_string(),
                "test_aircraft".to_string(),
                "1.0.0".to_string(),
            )
            .await
            .unwrap();

        writer.record_axis_frame(1000, b"test_data").unwrap();
        sleep(Duration::from_millis(50)).await;
        writer.stop_recording().await.unwrap();

        // Read and validate the file
        let mut reader = BlackboxReader::open(&filepath).unwrap();
        assert!(reader.validate().unwrap());

        // Check header
        let header = reader.header();
        assert_eq!(header.sim_id, "test_sim");
        assert_eq!(header.aircraft_id, "test_aircraft");
        assert_eq!(header.app_version, "1.0.0");
    }

    #[tokio::test]
    async fn test_blackbox_drop_handling() {
        let temp_dir = TempDir::new().unwrap();
        let config = BlackboxConfig {
            output_dir: temp_dir.path().to_path_buf(),
            buffer_size: 100, // Very small buffer to force drops
            ..Default::default()
        };

        let mut writer = BlackboxWriter::new(config);

        writer
            .start_recording(
                "test_sim".to_string(),
                "test_aircraft".to_string(),
                "1.0.0".to_string(),
            )
            .await
            .unwrap();

        // Flood with data to trigger drops
        for i in 0..1000 {
            let large_data = vec![0u8; 1000]; // 1KB per record
            writer.record_axis_frame(i * 1000, &large_data).unwrap();
        }

        sleep(Duration::from_millis(100)).await;
        let stats = writer.stop_recording().await.unwrap();

        // Should have some drops due to small buffer
        assert!(stats.drops_total > 0);
    }

    #[tokio::test]
    async fn test_blackbox_performance_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config = BlackboxConfig {
            output_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut writer = BlackboxWriter::new(config);

        let filepath = writer
            .start_recording(
                "perf_test".to_string(),
                "test_aircraft".to_string(),
                "1.0.0".to_string(),
            )
            .await
            .unwrap();

        let record_count_target = 5_000u64;
        for i in 0..record_count_target {
            let timestamp = i * 4_000_000;
            let axis_data = vec![0u8; 64]; // 64 bytes per axis frame
            writer.record_axis_frame(timestamp, &axis_data).unwrap();
        }

        let stats = writer.stop_recording().await.unwrap();

        // Verify zero drops for 1-second capture
        assert_eq!(
            stats.drops_total, 0,
            "Should have zero drops for 1-second capture"
        );

        assert_eq!(
            stats.records_written[StreamType::AxisFrames.to_index()],
            record_count_target
        );

        // Verify file size is reasonable
        let file_size = std::fs::metadata(&filepath).unwrap().len();
        assert!(
            file_size < 2 * 1024 * 1024,
            "File size should be reasonable for test capture"
        );

        println!("Performance test results:");
        println!("  Records: {}", record_count_target);
        println!("  File size: {} bytes", file_size);
        println!("  Drops: {}", stats.drops_total);
        println!("  Flush count: {}", stats.flush_count);
        println!("  Max flush duration: {} μs", stats.max_flush_duration_us);
    }
}
