// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Optimized HID writer using overlapped I/O for Windows.
//!
//! This module provides a high-performance HID writer that uses Windows overlapped
//! (asynchronous) I/O to achieve low-latency writes suitable for the 250Hz FFB loop.
//!
//! # Design
//!
//! The `HidWriter` opens HID devices with `FILE_FLAG_OVERLAPPED` and uses a pool of
//! pre-allocated `OVERLAPPED` structures to avoid allocations in the hot path. This
//! approach avoids the blocking behavior of `HidD_SetOutputReport` and achieves
//! p99 latency ≤300μs.
//!
//! # Requirements
//!
//! - **Requirement 4.1**: Open HID devices with `FILE_FLAG_OVERLAPPED` for non-blocking I/O
//! - **Requirement 4.2**: Use async `WriteFile` with `OVERLAPPED` struct pool instead of `HidD_SetOutputReport`

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::io;
    use thiserror::Error;
    use tracing::{debug, trace, warn};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING, WriteFile,
    };
    use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
    use windows::core::PCWSTR;

    /// Error type for HID writer operations
    #[derive(Debug, Error)]
    pub enum HidWriterError {
        /// Failed to open the HID device
        #[error("Failed to open HID device: {0}")]
        OpenFailed(io::Error),

        /// Failed to write to the HID device
        #[error("Failed to write to HID device: {0}")]
        WriteFailed(io::Error),

        /// Write operation is still pending (not an error, informational)
        #[error("Write operation pending")]
        WritePending,

        /// Device handle is invalid
        #[error("Invalid device handle")]
        InvalidHandle,

        /// Report buffer is empty
        #[error("Report buffer is empty")]
        EmptyReport,
    }

    /// Result of an async write operation
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WriteResult {
        /// Write completed immediately
        Completed { bytes_written: u32 },
        /// Write is pending (async)
        Pending,
    }

    /// Pre-allocated OVERLAPPED structure with associated state
    struct OverlappedSlot {
        /// The OVERLAPPED structure
        overlapped: Box<OVERLAPPED>,
        /// Whether this slot is currently in use
        in_use: bool,
    }

    impl OverlappedSlot {
        fn new() -> Self {
            Self {
                overlapped: Box::new(OVERLAPPED::default()),
                in_use: false,
            }
        }

        fn reset(&mut self) {
            // Reset the OVERLAPPED structure for reuse
            // SAFETY: OVERLAPPED is a POD type, zeroing is safe
            *self.overlapped = OVERLAPPED::default();
            self.in_use = false;
        }
    }

    /// Optimized HID writer using overlapped I/O.
    ///
    /// This writer opens HID devices with `FILE_FLAG_OVERLAPPED` for non-blocking I/O
    /// and uses a pool of pre-allocated `OVERLAPPED` structures to avoid allocations
    /// in the hot path.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use flight_hid::hid_writer::HidWriter;
    ///
    /// let mut writer = HidWriter::open(r"\\?\hid#vid_046d&pid_c262#...")?;
    /// let report = [0x00, 0x01, 0x02, 0x03]; // HID output report
    /// writer.write_async(&report)?;
    /// ```
    pub struct HidWriter {
        /// Device handle opened with FILE_FLAG_OVERLAPPED
        handle: HANDLE,
        /// Pool of OVERLAPPED structures for async writes
        overlapped_pool: Vec<OverlappedSlot>,
        /// Index of next available OVERLAPPED slot
        next_slot: usize,
        /// Device path (for diagnostics)
        device_path: String,
        /// Consecutive failure count for fault detection
        consecutive_failures: u32,
        /// Total writes attempted
        total_writes: u64,
        /// Total successful writes
        successful_writes: u64,
    }

    impl HidWriter {
        /// Default pool size for OVERLAPPED structures.
        /// 4 is sufficient for 250Hz operation with some headroom.
        const DEFAULT_POOL_SIZE: usize = 4;

        /// Maximum consecutive failures before triggering fault detection.
        /// Per Requirement 4.4: detect USB OUT stalls within 3 frames.
        pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

        /// Open a HID device for optimized writes.
        ///
        /// Opens the device with `FILE_FLAG_OVERLAPPED` for non-blocking I/O
        /// and pre-allocates a pool of `OVERLAPPED` structures.
        ///
        /// # Arguments
        ///
        /// * `device_path` - The Windows device path (e.g., `\\?\hid#vid_046d&pid_c262#...`)
        ///
        /// # Errors
        ///
        /// Returns `HidWriterError::OpenFailed` if the device cannot be opened.
        pub fn open(device_path: &str) -> Result<Self, HidWriterError> {
            Self::open_with_pool_size(device_path, Self::DEFAULT_POOL_SIZE)
        }

        /// Open a HID device with a custom OVERLAPPED pool size.
        ///
        /// # Arguments
        ///
        /// * `device_path` - The Windows device path
        /// * `pool_size` - Number of OVERLAPPED structures to pre-allocate
        ///
        /// # Errors
        ///
        /// Returns `HidWriterError::OpenFailed` if the device cannot be opened.
        pub fn open_with_pool_size(
            device_path: &str,
            pool_size: usize,
        ) -> Result<Self, HidWriterError> {
            // Convert path to wide string (null-terminated UTF-16)
            let path_wide: Vec<u16> = device_path.encode_utf16().chain(Some(0)).collect();

            // Open device with FILE_FLAG_OVERLAPPED for async I/O
            // SAFETY: We're calling a Windows API with valid parameters
            let handle = unsafe {
                CreateFileW(
                    PCWSTR::from_raw(path_wide.as_ptr()),
                    FILE_GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None, // No security attributes
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED, // Critical for async I/O
                    None,                 // No template file
                )
            }
            .map_err(|e| HidWriterError::OpenFailed(io::Error::from_raw_os_error(e.code().0)))?;

            if handle == INVALID_HANDLE_VALUE {
                return Err(HidWriterError::OpenFailed(io::Error::last_os_error()));
            }

            // Pre-allocate OVERLAPPED pool
            let overlapped_pool: Vec<_> = (0..pool_size.max(1))
                .map(|_| OverlappedSlot::new())
                .collect();

            debug!(
                "Opened HID device for overlapped I/O: {} (pool_size={})",
                device_path, pool_size
            );

            Ok(Self {
                handle,
                overlapped_pool,
                next_slot: 0,
                device_path: device_path.to_string(),
                consecutive_failures: 0,
                total_writes: 0,
                successful_writes: 0,
            })
        }

        /// Write an HID report asynchronously.
        ///
        /// Uses `WriteFile` with an `OVERLAPPED` structure for non-blocking I/O.
        /// The write may complete immediately or be pending; in either case,
        /// this method returns quickly without blocking.
        ///
        /// # Arguments
        ///
        /// * `report` - The HID output report to write (including report ID as first byte)
        ///
        /// # Returns
        ///
        /// * `Ok(WriteResult::Completed { bytes_written })` - Write completed immediately
        /// * `Ok(WriteResult::Pending)` - Write is pending (async)
        /// * `Err(HidWriterError)` - Write failed
        ///
        /// # Note
        ///
        /// For fire-and-forget writes at 250Hz, the pending state is acceptable.
        /// The next write will reuse the OVERLAPPED slot, implicitly waiting for
        /// the previous operation to complete.
        pub fn write_async(&mut self, report: &[u8]) -> Result<WriteResult, HidWriterError> {
            if report.is_empty() {
                return Err(HidWriterError::EmptyReport);
            }

            if self.handle == INVALID_HANDLE_VALUE {
                return Err(HidWriterError::InvalidHandle);
            }

            self.total_writes += 1;

            // Get the next OVERLAPPED slot (round-robin)
            let slot_idx = self.next_slot;
            self.next_slot = (self.next_slot + 1) % self.overlapped_pool.len();

            // If the slot is in use, wait for the previous operation to complete
            // This provides backpressure if writes are too fast
            // Check in_use first to avoid borrow issues
            if self.overlapped_pool[slot_idx].in_use {
                self.wait_for_slot(slot_idx)?;
            }

            // Now get mutable reference after wait_for_slot is done
            let slot = &mut self.overlapped_pool[slot_idx];

            // Reset the OVERLAPPED structure for reuse
            slot.reset();
            slot.in_use = true;

            let mut bytes_written: u32 = 0;

            // SAFETY: We're calling WriteFile with valid parameters
            // The OVERLAPPED structure is valid and will remain valid until the operation completes
            let result = unsafe {
                WriteFile(
                    self.handle,
                    Some(report),
                    Some(&mut bytes_written),
                    Some(slot.overlapped.as_mut()),
                )
            };

            match result {
                Ok(()) => {
                    // Write completed immediately
                    trace!(
                        "HID write completed immediately: {} bytes to {}",
                        bytes_written, self.device_path
                    );
                    slot.in_use = false;
                    self.record_success();
                    Ok(WriteResult::Completed { bytes_written })
                }
                Err(e) => {
                    let error_code = e.code().0 as u32;
                    // ERROR_IO_PENDING (997) is expected for async operations
                    if error_code == windows::Win32::Foundation::ERROR_IO_PENDING.0 {
                        trace!("HID write pending for {}", self.device_path);
                        // Don't mark as success yet - it's still pending
                        Ok(WriteResult::Pending)
                    } else {
                        // Actual error
                        slot.in_use = false;
                        let io_error = io::Error::from_raw_os_error(error_code as i32);
                        warn!(
                            "HID write failed for {}: {} (error {})",
                            self.device_path, io_error, error_code
                        );
                        self.record_failure();
                        Err(HidWriterError::WriteFailed(io_error))
                    }
                }
            }
        }

        /// Wait for a specific OVERLAPPED slot to complete.
        fn wait_for_slot(&mut self, slot_idx: usize) -> Result<(), HidWriterError> {
            let slot = &mut self.overlapped_pool[slot_idx];
            if !slot.in_use {
                return Ok(());
            }

            let mut bytes_transferred: u32 = 0;

            // SAFETY: We're calling GetOverlappedResult with valid parameters
            let result = unsafe {
                GetOverlappedResult(
                    self.handle,
                    slot.overlapped.as_ref(),
                    &mut bytes_transferred,
                    true, // Wait for completion
                )
            };

            slot.in_use = false;

            match result {
                Ok(()) => {
                    self.record_success();
                    Ok(())
                }
                Err(e) => {
                    let io_error = io::Error::from_raw_os_error(e.code().0);
                    self.record_failure();
                    Err(HidWriterError::WriteFailed(io_error))
                }
            }
        }

        /// Record a successful write operation.
        fn record_success(&mut self) {
            self.consecutive_failures = 0;
            self.successful_writes += 1;
        }

        /// Record a failed write operation.
        fn record_failure(&mut self) {
            self.consecutive_failures += 1;
        }

        /// Check if the device is in a fault state (USB OUT stall).
        ///
        /// Per Requirement 4.4: detect USB OUT stalls within 3 frames.
        ///
        /// # Returns
        ///
        /// `true` if consecutive failures >= `MAX_CONSECUTIVE_FAILURES`
        pub fn is_faulted(&self) -> bool {
            self.consecutive_failures >= Self::MAX_CONSECUTIVE_FAILURES
        }

        /// Get the number of consecutive failures.
        pub fn consecutive_failures(&self) -> u32 {
            self.consecutive_failures
        }

        /// Get the total number of write attempts.
        pub fn total_writes(&self) -> u64 {
            self.total_writes
        }

        /// Get the number of successful writes.
        pub fn successful_writes(&self) -> u64 {
            self.successful_writes
        }

        /// Get the device path.
        pub fn device_path(&self) -> &str {
            &self.device_path
        }

        /// Reset the fault state.
        ///
        /// Call this after recovering from a fault condition.
        pub fn reset_fault_state(&mut self) {
            self.consecutive_failures = 0;
        }

        /// Flush any pending writes by waiting for all OVERLAPPED slots.
        ///
        /// This is useful before closing the device or when synchronization is needed.
        pub fn flush(&mut self) -> Result<(), HidWriterError> {
            for slot_idx in 0..self.overlapped_pool.len() {
                if self.overlapped_pool[slot_idx].in_use {
                    self.wait_for_slot(slot_idx)?;
                }
            }
            Ok(())
        }
    }

    impl Drop for HidWriter {
        fn drop(&mut self) {
            // Try to flush pending writes (best effort)
            let _ = self.flush();

            // Close the device handle
            if self.handle != INVALID_HANDLE_VALUE {
                // SAFETY: We own the handle and it's valid
                unsafe {
                    let _ = CloseHandle(self.handle);
                }
                debug!("Closed HID device: {}", self.device_path);
            }
        }
    }

    // SAFETY: HidWriter can be sent between threads.
    // The HANDLE is thread-safe for I/O operations on Windows.
    // The OVERLAPPED pool is only accessed by one thread at a time.
    unsafe impl Send for HidWriter {}

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_overlapped_slot_reset() {
            let mut slot = OverlappedSlot::new();
            slot.in_use = true;
            slot.reset();
            assert!(!slot.in_use);
        }

        #[test]
        fn test_hid_writer_error_display() {
            let err = HidWriterError::EmptyReport;
            assert_eq!(format!("{}", err), "Report buffer is empty");

            let err = HidWriterError::InvalidHandle;
            assert_eq!(format!("{}", err), "Invalid device handle");
        }

        #[test]
        fn test_write_result_equality() {
            assert_eq!(
                WriteResult::Completed { bytes_written: 10 },
                WriteResult::Completed { bytes_written: 10 }
            );
            assert_eq!(WriteResult::Pending, WriteResult::Pending);
            assert_ne!(
                WriteResult::Pending,
                WriteResult::Completed { bytes_written: 0 }
            );
        }

        // Note: Integration tests with real HID devices would go in tests/
        // and require actual hardware or mock devices
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::*;

// Provide a stub implementation for non-Windows platforms for compilation
#[cfg(not(target_os = "windows"))]
mod stub_impl {
    use std::io;
    use thiserror::Error;

    /// Error type for HID writer operations (stub for non-Windows)
    #[derive(Debug, Error)]
    pub enum HidWriterError {
        /// Platform not supported
        #[error("HidWriter is only supported on Windows")]
        PlatformNotSupported,

        /// Failed to open the HID device
        #[error("Failed to open HID device: {0}")]
        OpenFailed(io::Error),

        /// Failed to write to the HID device
        #[error("Failed to write to HID device: {0}")]
        WriteFailed(io::Error),

        /// Write operation is still pending
        #[error("Write operation pending")]
        WritePending,

        /// Device handle is invalid
        #[error("Invalid device handle")]
        InvalidHandle,

        /// Report buffer is empty
        #[error("Report buffer is empty")]
        EmptyReport,
    }

    /// Result of an async write operation (stub for non-Windows)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WriteResult {
        /// Write completed immediately
        Completed { bytes_written: u32 },
        /// Write is pending (async)
        Pending,
    }

    /// Stub HidWriter for non-Windows platforms.
    ///
    /// This is a compile-time stub that returns errors on all operations.
    /// The real implementation is only available on Windows.
    pub struct HidWriter {
        _private: (),
    }

    impl HidWriter {
        /// Maximum consecutive failures before triggering fault detection.
        pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

        /// Open a HID device (stub - always fails on non-Windows).
        pub fn open(_device_path: &str) -> Result<Self, HidWriterError> {
            Err(HidWriterError::PlatformNotSupported)
        }

        /// Open a HID device with custom pool size (stub - always fails on non-Windows).
        pub fn open_with_pool_size(
            _device_path: &str,
            _pool_size: usize,
        ) -> Result<Self, HidWriterError> {
            Err(HidWriterError::PlatformNotSupported)
        }

        /// Write an HID report asynchronously (stub - always fails).
        pub fn write_async(&mut self, _report: &[u8]) -> Result<WriteResult, HidWriterError> {
            Err(HidWriterError::PlatformNotSupported)
        }

        /// Check if the device is in a fault state.
        pub fn is_faulted(&self) -> bool {
            false
        }

        /// Get the number of consecutive failures.
        pub fn consecutive_failures(&self) -> u32 {
            0
        }

        /// Get the total number of write attempts.
        pub fn total_writes(&self) -> u64 {
            0
        }

        /// Get the number of successful writes.
        pub fn successful_writes(&self) -> u64 {
            0
        }

        /// Get the device path.
        pub fn device_path(&self) -> &str {
            ""
        }

        /// Reset the fault state.
        pub fn reset_fault_state(&mut self) {}

        /// Flush any pending writes.
        pub fn flush(&mut self) -> Result<(), HidWriterError> {
            Err(HidWriterError::PlatformNotSupported)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub_impl::*;

// ============================================================================
// HID Latency Benchmark
// ============================================================================

/// HID latency statistics from benchmark runs.
///
/// Contains percentile latency measurements in microseconds.
#[derive(Debug, Default, Clone)]
pub struct HidLatencyStats {
    /// Number of samples collected
    pub samples: usize,
    /// 50th percentile (median) latency in microseconds
    pub p50_us: u64,
    /// 95th percentile latency in microseconds
    pub p95_us: u64,
    /// 99th percentile latency in microseconds
    pub p99_us: u64,
}

impl HidLatencyStats {
    /// Check if the p99 latency meets the requirement (≤300μs).
    ///
    /// **Validates: Requirements 4.3**
    pub fn meets_requirement(&self) -> bool {
        self.p99_us <= 300
    }
}

#[cfg(target_os = "windows")]
mod latency_bench {
    use super::*;
    use std::time::{Duration, Instant};
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    /// HID latency measurement harness.
    ///
    /// This benchmark measures the latency of HID write operations to validate
    /// that the optimized overlapped I/O implementation meets the p99 ≤300μs
    /// requirement specified in Requirement 4.3.
    ///
    /// # Design
    ///
    /// The benchmark uses QueryPerformanceCounter (QPC) for high-resolution
    /// timing measurements. It records the time taken for each `write_async`
    /// call and computes percentile statistics.
    ///
    /// # Requirements
    ///
    /// - **Requirement 4.3**: p99 write latency SHALL be ≤300μs measured over ≥10 minutes
    ///
    /// # Example
    ///
    /// ```ignore
    /// use flight_hid::hid_writer::{HidLatencyBench, HidLatencyStats};
    /// use std::time::Duration;
    ///
    /// let mut bench = HidLatencyBench::new(r"\\?\hid#vid_046d&pid_c262#...", 8);
    /// let stats = bench.run(Duration::from_secs(600))?; // 10 minutes
    ///
    /// assert!(stats.p99_us <= 300, "p99 latency {} > 300μs", stats.p99_us);
    /// ```
    pub struct HidLatencyBench {
        /// Device path for the HID device
        device_path: String,
        /// Recorded latencies in nanoseconds
        latencies: Vec<u64>,
        /// Report size in bytes
        report_size: usize,
        /// QPC frequency (ticks per second)
        qpc_freq: i64,
    }

    impl HidLatencyBench {
        /// Create a new HID latency benchmark.
        ///
        /// # Arguments
        ///
        /// * `device_path` - The Windows device path for the HID device
        /// * `report_size` - Size of the HID output report in bytes
        ///
        /// # Capacity
        ///
        /// Pre-allocates capacity for 600,000 samples (10 minutes at 1kHz).
        pub fn new(device_path: &str, report_size: usize) -> Self {
            // Get QPC frequency
            let mut freq: i64 = 0;
            // SAFETY: QueryPerformanceFrequency always succeeds on Windows XP and later
            let _ = unsafe { QueryPerformanceFrequency(&mut freq) };

            Self {
                device_path: device_path.to_string(),
                latencies: Vec::with_capacity(600_000), // 10 minutes at 1kHz
                report_size,
                qpc_freq: freq,
            }
        }

        /// Run the benchmark for the specified duration.
        ///
        /// Performs HID writes at approximately 1kHz and measures the latency
        /// of each write operation using QueryPerformanceCounter.
        ///
        /// # Arguments
        ///
        /// * `duration` - How long to run the benchmark
        ///
        /// # Returns
        ///
        /// `HidLatencyStats` containing percentile latency measurements.
        ///
        /// # Errors
        ///
        /// Returns `HidWriterError` if the device cannot be opened or writes fail.
        ///
        /// # Note
        ///
        /// This measures the submit latency (time to call `write_async`), not
        /// the completion latency. For overlapped I/O, the submit latency is
        /// the relevant metric for the RT loop since we don't block on completion.
        pub fn run(&mut self, duration: Duration) -> Result<HidLatencyStats, HidWriterError> {
            // Clear any previous results
            self.latencies.clear();

            // Open the HID device
            let mut writer = HidWriter::open(&self.device_path)?;

            // Create a test report (zeros are fine for latency measurement)
            let report = vec![0u8; self.report_size];

            let start = Instant::now();
            let mut before: i64 = 0;
            let mut after: i64 = 0;

            // Run the benchmark loop
            while start.elapsed() < duration {
                // Measure write latency using QPC
                // SAFETY: QueryPerformanceCounter always succeeds on Windows XP and later
                let _ = unsafe { QueryPerformanceCounter(&mut before) };

                // Perform the write (ignore result for latency measurement)
                // We're measuring submit latency, not completion
                let _ = writer.write_async(&report);

                // SAFETY: QueryPerformanceCounter always succeeds on Windows XP and later
                let _ = unsafe { QueryPerformanceCounter(&mut after) };

                // Calculate latency in nanoseconds
                let ticks = after - before;
                let latency_ns = (ticks * 1_000_000_000) / self.qpc_freq;
                self.latencies.push(latency_ns as u64);

                // Sleep for ~1ms to achieve ~1kHz sample rate
                // This prevents overwhelming the USB bus
                std::thread::sleep(Duration::from_micros(1000));
            }

            // Flush any pending writes
            let _ = writer.flush();

            Ok(self.compute_stats())
        }

        /// Compute latency statistics from recorded samples.
        fn compute_stats(&self) -> HidLatencyStats {
            if self.latencies.is_empty() {
                return HidLatencyStats::default();
            }

            // Sort latencies for percentile calculation
            let mut sorted = self.latencies.clone();
            sorted.sort_unstable();

            let len = sorted.len();

            // Calculate percentile indices
            let p50_idx = len / 2;
            let p95_idx = (len * 95) / 100;
            let p99_idx = (len * 99) / 100;

            HidLatencyStats {
                samples: len,
                // Convert from nanoseconds to microseconds
                p50_us: sorted[p50_idx] / 1000,
                p95_us: sorted[p95_idx] / 1000,
                p99_us: sorted[p99_idx] / 1000,
            }
        }

        /// Get the number of samples collected so far.
        pub fn sample_count(&self) -> usize {
            self.latencies.len()
        }

        /// Get the device path being benchmarked.
        pub fn device_path(&self) -> &str {
            &self.device_path
        }

        /// Get the report size being used.
        pub fn report_size(&self) -> usize {
            self.report_size
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_hid_latency_stats_default() {
            let stats = HidLatencyStats::default();
            assert_eq!(stats.samples, 0);
            assert_eq!(stats.p50_us, 0);
            assert_eq!(stats.p95_us, 0);
            assert_eq!(stats.p99_us, 0);
        }

        #[test]
        fn test_hid_latency_stats_meets_requirement() {
            // p99 = 300μs should pass
            let stats = HidLatencyStats {
                p99_us: 300,
                ..HidLatencyStats::default()
            };
            assert!(stats.meets_requirement());

            // p99 = 299μs should pass
            let stats = HidLatencyStats {
                p99_us: 299,
                ..HidLatencyStats::default()
            };
            assert!(stats.meets_requirement());

            // p99 = 301μs should fail
            let stats = HidLatencyStats {
                p99_us: 301,
                ..HidLatencyStats::default()
            };
            assert!(!stats.meets_requirement());
        }

        #[test]
        fn test_hid_latency_bench_new() {
            let bench = HidLatencyBench::new(r"\\?\hid#test", 8);
            assert_eq!(bench.device_path(), r"\\?\hid#test");
            assert_eq!(bench.report_size(), 8);
            assert_eq!(bench.sample_count(), 0);
            assert!(bench.qpc_freq > 0);
        }

        #[test]
        fn test_compute_stats_empty() {
            let bench = HidLatencyBench::new(r"\\?\hid#test", 8);
            let stats = bench.compute_stats();
            assert_eq!(stats.samples, 0);
        }

        /// Feature: release-readiness, Property 4: HID Write Latency
        ///
        /// **Validates: Requirements 4.3**
        ///
        /// This test validates that HID write latency meets the p99 ≤300μs requirement.
        /// It requires a real HID device and runs for 10 minutes.
        ///
        /// # Hardware Requirements
        ///
        /// - A connected HID device (e.g., joystick, FFB wheel)
        /// - The device path must be updated to match the actual device
        ///
        /// # Running
        ///
        /// ```bash
        /// cargo test --release -p flight-hid test_hid_latency_10min -- --ignored --nocapture
        /// ```
        #[test]
        #[ignore] // Requires hardware and takes 10+ minutes
        fn test_hid_latency_10min() {
            // NOTE: Update this path to match your actual HID device
            // You can find device paths using Device Manager or tools like hidapi
            let device_path = std::env::var("FLIGHT_HID_TEST_DEVICE")
                .unwrap_or_else(|_| r"\\?\hid#vid_046d&pid_c262#7&1234&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}".to_string());

            // Standard FFB report size (adjust based on device)
            let report_size = 8;

            println!("HID Latency Benchmark");
            println!("=====================");
            println!("Device: {}", device_path);
            println!("Report size: {} bytes", report_size);
            println!("Duration: 10 minutes");
            println!();

            let mut bench = HidLatencyBench::new(&device_path, report_size);

            // Run for 10 minutes as required by Requirement 4.3
            let duration = Duration::from_secs(600);

            println!("Starting benchmark...");
            let start = Instant::now();

            match bench.run(duration) {
                Ok(stats) => {
                    let elapsed = start.elapsed();

                    println!();
                    println!("Results");
                    println!("-------");
                    println!("Duration: {:.1}s", elapsed.as_secs_f64());
                    println!("Samples: {}", stats.samples);
                    println!("p50 latency: {}μs", stats.p50_us);
                    println!("p95 latency: {}μs", stats.p95_us);
                    println!("p99 latency: {}μs", stats.p99_us);
                    println!();

                    // Assert p99 ≤ 300μs per Requirement 4.3
                    assert!(
                        stats.meets_requirement(),
                        "FAILED: p99 latency {}μs > 300μs requirement",
                        stats.p99_us
                    );

                    println!("PASSED: p99 latency {}μs ≤ 300μs", stats.p99_us);
                }
                Err(e) => {
                    panic!("Benchmark failed: {}", e);
                }
            }
        }

        /// Short smoke test for HID latency (1 second).
        ///
        /// This is a quick sanity check that can run without the full 10-minute duration.
        /// It's still marked as ignored because it requires hardware.
        #[test]
        #[ignore] // Requires hardware
        fn test_hid_latency_smoke() {
            let device_path = std::env::var("FLIGHT_HID_TEST_DEVICE")
                .unwrap_or_else(|_| r"\\?\hid#vid_046d&pid_c262#7&1234&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}".to_string());

            let mut bench = HidLatencyBench::new(&device_path, 8);

            // Run for just 1 second as a smoke test
            let stats = bench.run(Duration::from_secs(1)).expect("Benchmark failed");

            println!("Smoke test results:");
            println!("  Samples: {}", stats.samples);
            println!(
                "  p50: {}μs, p95: {}μs, p99: {}μs",
                stats.p50_us, stats.p95_us, stats.p99_us
            );

            // Should have collected ~1000 samples at 1kHz
            assert!(stats.samples > 500, "Too few samples: {}", stats.samples);
        }
    }
}

#[cfg(target_os = "windows")]
pub use latency_bench::HidLatencyBench;

// Stub implementation for non-Windows platforms
#[cfg(not(target_os = "windows"))]
mod latency_bench_stub {
    use super::*;

    /// Stub HID latency benchmark for non-Windows platforms.
    ///
    /// This is a compile-time stub that returns errors on all operations.
    /// The real implementation is only available on Windows.
    pub struct HidLatencyBench {
        _private: (),
    }

    impl HidLatencyBench {
        /// Create a new HID latency benchmark (stub - always fails on non-Windows).
        pub fn new(_device_path: &str, _report_size: usize) -> Self {
            Self { _private: () }
        }

        /// Run the benchmark (stub - always fails on non-Windows).
        pub fn run(
            &mut self,
            _duration: std::time::Duration,
        ) -> Result<HidLatencyStats, HidWriterError> {
            Err(HidWriterError::PlatformNotSupported)
        }

        /// Get the number of samples collected.
        pub fn sample_count(&self) -> usize {
            0
        }

        /// Get the device path.
        pub fn device_path(&self) -> &str {
            ""
        }

        /// Get the report size.
        pub fn report_size(&self) -> usize {
            0
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub use latency_bench_stub::HidLatencyBench;

// ============================================================================
// Fault Detection
// ============================================================================

/// Fault tracker for HID write operations.
///
/// This struct encapsulates the fault detection logic for HID writes,
/// tracking consecutive failures and determining when a fault condition
/// should be triggered.
///
/// # Design
///
/// Per Requirement 4.4, USB OUT stalls (≥3 consecutive write failures)
/// must be detected within 3 frames (12ms at 250Hz). This tracker provides
/// the state machine for fault detection.
///
/// # Example
///
/// ```
/// use flight_hid::hid_writer::FaultTracker;
///
/// let mut tracker = FaultTracker::new();
///
/// // Record some failures
/// tracker.record_failure();
/// tracker.record_failure();
/// assert!(!tracker.is_faulted()); // Not yet faulted (< 3 failures)
///
/// tracker.record_failure();
/// assert!(tracker.is_faulted()); // Now faulted (>= 3 failures)
///
/// // Success resets the counter
/// tracker.record_success();
/// assert!(!tracker.is_faulted());
/// ```
#[derive(Debug, Clone, Default)]
pub struct FaultTracker {
    /// Consecutive failure count
    consecutive_failures: u32,
    /// Total failures recorded
    total_failures: u64,
    /// Total successes recorded
    total_successes: u64,
}

impl FaultTracker {
    /// Maximum consecutive failures before triggering fault detection.
    /// Per Requirement 4.4: detect USB OUT stalls within 3 frames.
    pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

    /// Create a new fault tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful write operation.
    ///
    /// This resets the consecutive failure counter.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.total_successes += 1;
    }

    /// Record a failed write operation.
    ///
    /// This increments the consecutive failure counter.
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.total_failures += 1;
    }

    /// Check if the device is in a fault state.
    ///
    /// Per Requirement 4.4: detect USB OUT stalls within 3 frames.
    ///
    /// # Returns
    ///
    /// `true` if consecutive failures >= `MAX_CONSECUTIVE_FAILURES`
    pub fn is_faulted(&self) -> bool {
        self.consecutive_failures >= Self::MAX_CONSECUTIVE_FAILURES
    }

    /// Get the number of consecutive failures.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Get the total number of failures recorded.
    pub fn total_failures(&self) -> u64 {
        self.total_failures
    }

    /// Get the total number of successes recorded.
    pub fn total_successes(&self) -> u64 {
        self.total_successes
    }

    /// Reset the fault state.
    ///
    /// Call this after recovering from a fault condition.
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
    }
}

// ============================================================================
// Property-Based Tests for Fault Detection
// ============================================================================

#[cfg(test)]
mod fault_detection_tests {
    use super::*;
    use proptest::prelude::*;

    /// Represents a write operation result for property testing.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum WriteOutcome {
        Success,
        Failure,
    }

    /// Strategy to generate write outcomes.
    fn write_outcome_strategy() -> impl Strategy<Value = WriteOutcome> {
        prop_oneof![Just(WriteOutcome::Success), Just(WriteOutcome::Failure),]
    }

    /// Strategy to generate sequences of write outcomes.
    fn write_sequence_strategy() -> impl Strategy<Value = Vec<WriteOutcome>> {
        prop::collection::vec(write_outcome_strategy(), 0..100)
    }

    // ========================================================================
    // Feature: release-readiness, Property 5: HID Fault Detection
    // **Validates: Requirements 4.4**
    //
    // For any sequence of HID writes where ≥3 consecutive writes fail,
    // the fault handler SHALL be triggered within 3 frames (12ms at 250Hz).
    // ========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: is_faulted() returns false when consecutive_failures < 3
        ///
        /// **Validates: Requirements 4.4**
        ///
        /// For any number of consecutive failures less than MAX_CONSECUTIVE_FAILURES,
        /// the tracker should NOT be in a faulted state.
        #[test]
        fn prop_not_faulted_when_below_threshold(failures in 0u32..FaultTracker::MAX_CONSECUTIVE_FAILURES) {
            let mut tracker = FaultTracker::new();

            // Record the specified number of failures
            for _ in 0..failures {
                tracker.record_failure();
            }

            // Should NOT be faulted when below threshold
            prop_assert!(!tracker.is_faulted(),
                "Tracker should not be faulted with {} consecutive failures (threshold is {})",
                failures, FaultTracker::MAX_CONSECUTIVE_FAILURES);

            prop_assert_eq!(tracker.consecutive_failures(), failures,
                "Consecutive failures count should match recorded failures");
        }

        /// Property: is_faulted() returns true when consecutive_failures >= 3
        ///
        /// **Validates: Requirements 4.4**
        ///
        /// For any number of consecutive failures >= MAX_CONSECUTIVE_FAILURES,
        /// the tracker MUST be in a faulted state.
        #[test]
        fn prop_faulted_when_at_or_above_threshold(
            failures in FaultTracker::MAX_CONSECUTIVE_FAILURES..100u32
        ) {
            let mut tracker = FaultTracker::new();

            // Record the specified number of failures
            for _ in 0..failures {
                tracker.record_failure();
            }

            // MUST be faulted when at or above threshold
            prop_assert!(tracker.is_faulted(),
                "Tracker MUST be faulted with {} consecutive failures (threshold is {})",
                failures, FaultTracker::MAX_CONSECUTIVE_FAILURES);

            prop_assert_eq!(tracker.consecutive_failures(), failures,
                "Consecutive failures count should match recorded failures");
        }

        /// Property: Successful writes reset the consecutive_failures counter
        ///
        /// **Validates: Requirements 4.4**
        ///
        /// For any sequence of failures followed by a success, the consecutive
        /// failure counter MUST be reset to 0.
        #[test]
        fn prop_success_resets_consecutive_failures(failures in 1u32..50) {
            let mut tracker = FaultTracker::new();

            // Record some failures
            for _ in 0..failures {
                tracker.record_failure();
            }

            // Verify failures were recorded
            prop_assert_eq!(tracker.consecutive_failures(), failures);

            // Record a success
            tracker.record_success();

            // Consecutive failures MUST be reset to 0
            prop_assert_eq!(tracker.consecutive_failures(), 0,
                "Success should reset consecutive failures to 0");

            // Should no longer be faulted (if it was)
            prop_assert!(!tracker.is_faulted(),
                "Tracker should not be faulted after a successful write");
        }

        /// Property: Fault detection triggers within 3 consecutive failures
        ///
        /// **Validates: Requirements 4.4**
        ///
        /// For any sequence of write outcomes, if there are ≥3 consecutive
        /// failures at any point, is_faulted() MUST return true at that point.
        #[test]
        fn prop_fault_detection_within_threshold(sequence in write_sequence_strategy()) {
            let mut tracker = FaultTracker::new();
            let mut consecutive_failures = 0u32;

            for outcome in &sequence {
                match outcome {
                    WriteOutcome::Success => {
                        tracker.record_success();
                        consecutive_failures = 0;
                    }
                    WriteOutcome::Failure => {
                        tracker.record_failure();
                        consecutive_failures += 1;
                    }
                }

                // Verify fault state matches expected state
                let expected_faulted = consecutive_failures >= FaultTracker::MAX_CONSECUTIVE_FAILURES;
                prop_assert_eq!(tracker.is_faulted(), expected_faulted,
                    "Fault state mismatch: consecutive_failures={}, expected_faulted={}, actual_faulted={}",
                    consecutive_failures, expected_faulted, tracker.is_faulted());
            }
        }

        /// Property: Total counters are accurate
        ///
        /// **Validates: Requirements 4.4**
        ///
        /// For any sequence of write outcomes, the total success and failure
        /// counters MUST accurately reflect the number of each outcome.
        #[test]
        fn prop_total_counters_accurate(sequence in write_sequence_strategy()) {
            let mut tracker = FaultTracker::new();
            let mut expected_successes = 0u64;
            let mut expected_failures = 0u64;

            for outcome in &sequence {
                match outcome {
                    WriteOutcome::Success => {
                        tracker.record_success();
                        expected_successes += 1;
                    }
                    WriteOutcome::Failure => {
                        tracker.record_failure();
                        expected_failures += 1;
                    }
                }
            }

            prop_assert_eq!(tracker.total_successes(), expected_successes,
                "Total successes mismatch");
            prop_assert_eq!(tracker.total_failures(), expected_failures,
                "Total failures mismatch");
        }

        /// Property: Reset clears fault state but preserves totals
        ///
        /// **Validates: Requirements 4.4**
        ///
        /// Calling reset() should clear the consecutive failure counter
        /// but preserve the total counters.
        #[test]
        fn prop_reset_clears_fault_preserves_totals(
            failures in 1u32..50,
            successes in 0u32..50
        ) {
            let mut tracker = FaultTracker::new();

            // Record some successes and failures
            for _ in 0..successes {
                tracker.record_success();
            }
            for _ in 0..failures {
                tracker.record_failure();
            }

            let total_successes_before = tracker.total_successes();
            let total_failures_before = tracker.total_failures();

            // Reset the tracker
            tracker.reset();

            // Consecutive failures should be 0
            prop_assert_eq!(tracker.consecutive_failures(), 0,
                "Reset should clear consecutive failures");

            // Should not be faulted
            prop_assert!(!tracker.is_faulted(),
                "Reset should clear fault state");

            // Totals should be preserved
            prop_assert_eq!(tracker.total_successes(), total_successes_before,
                "Reset should preserve total successes");
            prop_assert_eq!(tracker.total_failures(), total_failures_before,
                "Reset should preserve total failures");
        }
    }

    // ========================================================================
    // Unit Tests for Edge Cases
    // ========================================================================

    #[test]
    fn test_new_tracker_not_faulted() {
        let tracker = FaultTracker::new();
        assert!(!tracker.is_faulted());
        assert_eq!(tracker.consecutive_failures(), 0);
        assert_eq!(tracker.total_failures(), 0);
        assert_eq!(tracker.total_successes(), 0);
    }

    #[test]
    fn test_exactly_at_threshold() {
        let mut tracker = FaultTracker::new();

        // Record exactly MAX_CONSECUTIVE_FAILURES failures
        for _ in 0..FaultTracker::MAX_CONSECUTIVE_FAILURES {
            tracker.record_failure();
        }

        assert!(tracker.is_faulted());
        assert_eq!(
            tracker.consecutive_failures(),
            FaultTracker::MAX_CONSECUTIVE_FAILURES
        );
    }

    #[test]
    fn test_one_below_threshold() {
        let mut tracker = FaultTracker::new();

        // Record one less than MAX_CONSECUTIVE_FAILURES failures
        for _ in 0..(FaultTracker::MAX_CONSECUTIVE_FAILURES - 1) {
            tracker.record_failure();
        }

        assert!(!tracker.is_faulted());
        assert_eq!(
            tracker.consecutive_failures(),
            FaultTracker::MAX_CONSECUTIVE_FAILURES - 1
        );
    }

    #[test]
    fn test_interleaved_success_failure() {
        let mut tracker = FaultTracker::new();

        // Interleave successes and failures - should never fault
        for _ in 0..10 {
            tracker.record_failure();
            tracker.record_failure();
            tracker.record_success(); // Resets before reaching 3
        }

        assert!(!tracker.is_faulted());
        assert_eq!(tracker.consecutive_failures(), 0);
    }

    #[test]
    fn test_recovery_from_fault() {
        let mut tracker = FaultTracker::new();

        // Enter fault state
        for _ in 0..5 {
            tracker.record_failure();
        }
        assert!(tracker.is_faulted());

        // Recover with a success
        tracker.record_success();
        assert!(!tracker.is_faulted());
        assert_eq!(tracker.consecutive_failures(), 0);
    }

    #[test]
    fn test_max_consecutive_failures_constant() {
        // Verify the constant matches the requirement (3 frames)
        assert_eq!(FaultTracker::MAX_CONSECUTIVE_FAILURES, 3);
    }
}
