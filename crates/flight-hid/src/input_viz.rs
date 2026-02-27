use std::collections::VecDeque;

/// State of a single axis at a point in time.
#[derive(Debug, Clone)]
pub struct AxisState {
    pub index: u8,
    pub raw: f64,
    pub calibrated: f64,
    pub name: Option<String>,
}

/// State of a single button at a point in time.
#[derive(Debug, Clone)]
pub struct ButtonState {
    pub index: u8,
    pub pressed: bool,
    pub name: Option<String>,
}

/// A complete snapshot of all device inputs at one instant.
#[derive(Debug, Clone)]
pub struct InputSnapshot {
    pub device_id: String,
    pub axes: Vec<AxisState>,
    pub buttons: Vec<ButtonState>,
    pub timestamp_us: u64,
}

/// Rolling history of input snapshots for UI visualisation.
pub struct InputHistory {
    snapshots: VecDeque<InputSnapshot>,
    max_history: usize,
}

impl InputHistory {
    /// Create a new history buffer with the given capacity.
    #[must_use]
    pub fn new(max_history: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Record a new snapshot, evicting the oldest if at capacity.
    pub fn record(&mut self, snapshot: InputSnapshot) {
        if self.snapshots.len() >= self.max_history {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    /// Return the most recent snapshot, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&InputSnapshot> {
        self.snapshots.back()
    }

    /// Return the calibrated values for a given axis index across all
    /// stored snapshots (oldest first).
    #[must_use]
    pub fn history_for_axis(&self, axis_index: u8) -> Vec<(u64, f64)> {
        self.snapshots
            .iter()
            .filter_map(|snap| {
                snap.axes
                    .iter()
                    .find(|a| a.index == axis_index)
                    .map(|a| (snap.timestamp_us, a.calibrated))
            })
            .collect()
    }

    /// Return the (min, max) calibrated range observed for `axis_index`.
    #[must_use]
    pub fn axis_range(&self, axis_index: u8) -> Option<(f64, f64)> {
        let values: Vec<f64> = self
            .snapshots
            .iter()
            .filter_map(|snap| {
                snap.axes
                    .iter()
                    .find(|a| a.index == axis_index)
                    .map(|a| a.calibrated)
            })
            .collect();
        if values.is_empty() {
            return None;
        }
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Some((min, max))
    }

    /// Number of snapshots currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(ts: u64, axis_val: f64) -> InputSnapshot {
        InputSnapshot {
            device_id: "dev1".into(),
            axes: vec![AxisState {
                index: 0,
                raw: axis_val,
                calibrated: axis_val,
                name: None,
            }],
            buttons: vec![ButtonState {
                index: 0,
                pressed: false,
                name: None,
            }],
            timestamp_us: ts,
        }
    }

    #[test]
    fn empty_history() {
        let h = InputHistory::new(10);
        assert!(h.is_empty());
        assert!(h.latest().is_none());
    }

    #[test]
    fn record_and_latest() {
        let mut h = InputHistory::new(10);
        h.record(snap(100, 0.5));
        assert_eq!(h.len(), 1);
        assert_eq!(h.latest().unwrap().timestamp_us, 100);
    }

    #[test]
    fn eviction_at_capacity() {
        let mut h = InputHistory::new(3);
        h.record(snap(1, 0.1));
        h.record(snap(2, 0.2));
        h.record(snap(3, 0.3));
        h.record(snap(4, 0.4));
        assert_eq!(h.len(), 3);
        // Oldest (ts=1) should have been evicted
        assert_eq!(h.snapshots.front().unwrap().timestamp_us, 2);
    }

    #[test]
    fn history_for_axis() {
        let mut h = InputHistory::new(10);
        h.record(snap(1, 0.1));
        h.record(snap(2, 0.5));
        h.record(snap(3, 0.9));
        let hist = h.history_for_axis(0);
        assert_eq!(hist.len(), 3);
        assert!((hist[0].1 - 0.1).abs() < f64::EPSILON);
        assert!((hist[2].1 - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn history_for_missing_axis() {
        let mut h = InputHistory::new(10);
        h.record(snap(1, 0.5));
        let hist = h.history_for_axis(99);
        assert!(hist.is_empty());
    }

    #[test]
    fn axis_range_single_value() {
        let mut h = InputHistory::new(10);
        h.record(snap(1, 0.42));
        let (min, max) = h.axis_range(0).unwrap();
        assert!((min - 0.42).abs() < f64::EPSILON);
        assert!((max - 0.42).abs() < f64::EPSILON);
    }

    #[test]
    fn axis_range_multiple_values() {
        let mut h = InputHistory::new(10);
        h.record(snap(1, -1.0));
        h.record(snap(2, 0.0));
        h.record(snap(3, 1.0));
        let (min, max) = h.axis_range(0).unwrap();
        assert!((min - (-1.0)).abs() < f64::EPSILON);
        assert!((max - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn axis_range_missing_axis() {
        let h = InputHistory::new(10);
        assert!(h.axis_range(0).is_none());
    }

    #[test]
    fn multiple_axes_in_snapshot() {
        let mut h = InputHistory::new(10);
        h.record(InputSnapshot {
            device_id: "dev".into(),
            axes: vec![
                AxisState {
                    index: 0,
                    raw: 0.0,
                    calibrated: 0.1,
                    name: Some("X".into()),
                },
                AxisState {
                    index: 1,
                    raw: 0.0,
                    calibrated: 0.9,
                    name: Some("Y".into()),
                },
            ],
            buttons: vec![],
            timestamp_us: 1,
        });
        assert_eq!(h.history_for_axis(0).len(), 1);
        assert_eq!(h.history_for_axis(1).len(), 1);
        assert!((h.axis_range(1).unwrap().0 - 0.9).abs() < f64::EPSILON);
    }
}
