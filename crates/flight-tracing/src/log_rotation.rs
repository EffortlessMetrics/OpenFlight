/// Configuration for log file rotation.
#[derive(Debug, Clone)]
pub struct RotationConfig {
    /// Maximum size in bytes before a rotation is triggered.
    pub max_file_size_bytes: u64,
    /// Maximum number of rotated log files to keep.
    pub max_files: u32,
    /// Whether rotated files should be compressed.
    pub compress_rotated: bool,
}

/// Outcome of a single rotation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationResult {
    /// Rotation completed successfully.
    Rotated {
        /// The sequence number of the new rotated file.
        sequence: u32,
    },
    /// The file was not large enough to warrant rotation.
    NotNeeded,
    /// Rotation could not proceed because `max_files` has been reached.
    MaxFilesReached,
}

/// Pure-logic log rotation tracker (no file I/O).
pub struct LogRotator {
    config: RotationConfig,
    current_size: u64,
    rotation_count: u32,
}

impl LogRotator {
    /// Create a new rotator with the given configuration.
    #[must_use]
    pub fn new(config: RotationConfig) -> Self {
        Self {
            config,
            current_size: 0,
            rotation_count: 0,
        }
    }

    /// Returns `true` when the current virtual file has reached the size limit.
    #[must_use]
    pub fn should_rotate(&self) -> bool {
        self.current_size >= self.config.max_file_size_bytes
    }

    /// Record that `n` bytes were written to the current log file.
    pub fn record_bytes(&mut self, n: u64) {
        self.current_size = self.current_size.saturating_add(n);
    }

    /// Perform a rotation (resets the current size counter).
    pub fn rotate(&mut self) -> RotationResult {
        if !self.should_rotate() {
            return RotationResult::NotNeeded;
        }
        if self.rotation_count >= self.config.max_files {
            return RotationResult::MaxFilesReached;
        }
        self.rotation_count += 1;
        self.current_size = 0;
        RotationResult::Rotated {
            sequence: self.rotation_count,
        }
    }

    /// Number of rotations performed so far.
    #[must_use]
    pub fn rotation_count(&self) -> u32 {
        self.rotation_count
    }

    /// Current tracked file size.
    #[must_use]
    pub fn current_size(&self) -> u64 {
        self.current_size
    }

    /// Whether compression is enabled for rotated files.
    #[must_use]
    pub fn compress_enabled(&self) -> bool {
        self.config.compress_rotated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(size: u64, max_files: u32) -> RotationConfig {
        RotationConfig {
            max_file_size_bytes: size,
            max_files,
            compress_rotated: false,
        }
    }

    #[test]
    fn new_rotator_empty() {
        let r = LogRotator::new(cfg(1024, 5));
        assert_eq!(r.current_size(), 0);
        assert_eq!(r.rotation_count(), 0);
        assert!(!r.should_rotate());
    }

    #[test]
    fn should_rotate_at_threshold() {
        let mut r = LogRotator::new(cfg(100, 5));
        r.record_bytes(99);
        assert!(!r.should_rotate());
        r.record_bytes(1);
        assert!(r.should_rotate());
    }

    #[test]
    fn rotate_resets_size() {
        let mut r = LogRotator::new(cfg(100, 5));
        r.record_bytes(100);
        let res = r.rotate();
        assert_eq!(res, RotationResult::Rotated { sequence: 1 });
        assert_eq!(r.current_size(), 0);
    }

    #[test]
    fn rotate_when_not_needed() {
        let mut r = LogRotator::new(cfg(100, 5));
        r.record_bytes(50);
        assert_eq!(r.rotate(), RotationResult::NotNeeded);
    }

    #[test]
    fn max_files_limit() {
        let mut r = LogRotator::new(cfg(10, 2));
        r.record_bytes(10);
        assert_eq!(r.rotate(), RotationResult::Rotated { sequence: 1 });
        r.record_bytes(10);
        assert_eq!(r.rotate(), RotationResult::Rotated { sequence: 2 });
        r.record_bytes(10);
        assert_eq!(r.rotate(), RotationResult::MaxFilesReached);
    }

    #[test]
    fn rotation_count_increments() {
        let mut r = LogRotator::new(cfg(10, 10));
        for i in 1..=5 {
            r.record_bytes(10);
            r.rotate();
            assert_eq!(r.rotation_count(), i);
        }
    }

    #[test]
    fn record_bytes_accumulates() {
        let mut r = LogRotator::new(cfg(1000, 5));
        r.record_bytes(100);
        r.record_bytes(200);
        assert_eq!(r.current_size(), 300);
    }

    #[test]
    fn saturating_add_no_overflow() {
        let mut r = LogRotator::new(cfg(u64::MAX, 5));
        r.record_bytes(u64::MAX);
        r.record_bytes(1);
        assert_eq!(r.current_size(), u64::MAX);
    }

    #[test]
    fn compress_flag() {
        let c = RotationConfig {
            max_file_size_bytes: 100,
            max_files: 3,
            compress_rotated: true,
        };
        let r = LogRotator::new(c);
        assert!(r.compress_enabled());
    }
}
