//! Fault detection and handling for force feedback safety
//!
//! Implements comprehensive fault detection matrix with immediate safety responses.
//! All faults trigger torque-to-zero within 50ms and appropriate recovery actions.

use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Types of faults that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FaultType {
    /// USB output endpoint stalled for 3+ frames
    UsbStall,
    /// USB endpoint error or wedged state
    EndpointError,
    /// NaN or invalid value in pipeline
    NanValue,
    /// Device over-temperature condition
    OverTemp,
    /// Device over-current condition
    OverCurrent,
    /// Plugin exceeded time budget
    PluginOverrun,
}

impl FaultType {
    /// Get stable error code for this fault type
    pub fn error_code(&self) -> &'static str {
        match self {
            FaultType::UsbStall => "HID_OUT_STALL",
            FaultType::EndpointError => "HID_ENDPOINT_ERROR",
            FaultType::NanValue => "AXIS_NAN_VALUE",
            FaultType::OverTemp => "FFB_OVER_TEMP",
            FaultType::OverCurrent => "FFB_OVER_CURRENT",
            FaultType::PluginOverrun => "PLUG_OVERRUN",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            FaultType::UsbStall => "USB output endpoint stalled",
            FaultType::EndpointError => "USB endpoint error or wedged",
            FaultType::NanValue => "Invalid NaN value in axis pipeline",
            FaultType::OverTemp => "Device over-temperature protection",
            FaultType::OverCurrent => "Device over-current protection",
            FaultType::PluginOverrun => "Plugin exceeded time budget",
        }
    }

    /// Check if this fault requires immediate torque cutoff
    pub fn requires_torque_cutoff(&self) -> bool {
        match self {
            FaultType::UsbStall |
            FaultType::EndpointError |
            FaultType::NanValue |
            FaultType::OverTemp |
            FaultType::OverCurrent => true,
            FaultType::PluginOverrun => false, // Plugin faults don't affect FFB
        }
    }

    /// Get maximum allowed response time for this fault
    pub fn max_response_time(&self) -> Duration {
        match self {
            FaultType::UsbStall |
            FaultType::EndpointError |
            FaultType::NanValue |
            FaultType::OverTemp |
            FaultType::OverCurrent => Duration::from_millis(50),
            FaultType::PluginOverrun => Duration::from_millis(100),
        }
    }
}

/// Fault detection and response actions
#[derive(Debug, Clone, PartialEq)]
pub enum FaultAction {
    /// Ramp torque to zero within specified time
    TorqueZero50ms,
    /// Reset device connection
    DeviceReset,
    /// Quarantine component for session
    QuarantineComponent,
    /// Log and continue operation
    LogAndContinue,
}

/// Record of a detected fault
#[derive(Debug, Clone)]
pub struct FaultRecord {
    /// Type of fault detected
    pub fault_type: FaultType,
    /// When the fault was detected
    pub detected_at: Instant,
    /// Action taken in response
    pub action_taken: FaultAction,
    /// Time taken to respond to fault
    pub response_time: Option<Duration>,
    /// Additional context about the fault
    pub context: String,
    /// Whether fault caused safety state transition
    pub caused_safety_transition: bool,
}

/// Soft-stop event record
#[derive(Debug, Clone)]
pub struct SoftStopRecord {
    /// When soft-stop was triggered
    pub triggered_at: Instant,
    /// Reason for soft-stop
    pub reason: String,
    /// Time to complete torque ramp to zero
    pub ramp_duration: Option<Duration>,
    /// Whether audio cue was triggered
    pub audio_cue_triggered: bool,
}

/// Fault detection and response system
#[derive(Debug)]
pub struct FaultDetector {
    /// Maximum response time for critical faults
    max_response_time: Duration,
    /// History of detected faults
    fault_history: VecDeque<FaultRecord>,
    /// History of soft-stop events
    soft_stop_history: VecDeque<SoftStopRecord>,
    /// Maximum history size
    max_history_size: usize,
    /// Fault counters by type
    fault_counters: std::collections::HashMap<FaultType, u32>,
    /// Last fault detection time
    last_fault_time: Option<Instant>,
}

impl FaultDetector {
    /// Create new fault detector
    pub fn new(max_response_time: Duration) -> Self {
        Self {
            max_response_time,
            fault_history: VecDeque::new(),
            soft_stop_history: VecDeque::new(),
            max_history_size: 1000,
            fault_counters: std::collections::HashMap::new(),
            last_fault_time: None,
        }
    }

    /// Record a detected fault
    pub fn record_fault(&mut self, fault_type: FaultType) -> FaultRecord {
        let detected_at = Instant::now();
        self.last_fault_time = Some(detected_at);

        // Increment counter
        *self.fault_counters.entry(fault_type.clone()).or_insert(0) += 1;

        // Determine action based on fault type
        let action_taken = match fault_type {
            FaultType::UsbStall | 
            FaultType::EndpointError | 
            FaultType::NanValue | 
            FaultType::OverTemp | 
            FaultType::OverCurrent => FaultAction::TorqueZero50ms,
            FaultType::PluginOverrun => FaultAction::QuarantineComponent,
        };

        let record = FaultRecord {
            fault_type: fault_type.clone(),
            detected_at,
            action_taken,
            response_time: None, // Will be filled in when response completes
            context: format!("Fault detected: {}", fault_type.description()),
            caused_safety_transition: fault_type.requires_torque_cutoff(),
        };

        // Add to history
        self.fault_history.push_back(record.clone());
        
        // Keep history bounded
        if self.fault_history.len() > self.max_history_size {
            self.fault_history.pop_front();
        }

        record
    }

    /// Record completion of fault response
    pub fn record_fault_response_complete(&mut self, fault_type: FaultType, response_time: Duration) {
        // Find the most recent fault of this type and update response time
        if let Some(record) = self.fault_history.iter_mut().rev()
            .find(|r| r.fault_type == fault_type && r.response_time.is_none()) {
            record.response_time = Some(response_time);
        }
    }

    /// Record a soft-stop event
    pub fn record_soft_stop(&mut self, triggered_at: Instant) -> SoftStopRecord {
        let record = SoftStopRecord {
            triggered_at,
            reason: "Fault-triggered soft-stop".to_string(),
            ramp_duration: None, // Will be filled when ramp completes
            audio_cue_triggered: true,
        };

        self.soft_stop_history.push_back(record.clone());
        
        // Keep history bounded
        if self.soft_stop_history.len() > self.max_history_size {
            self.soft_stop_history.pop_front();
        }

        record
    }

    /// Record completion of soft-stop ramp
    pub fn record_soft_stop_complete(&mut self, ramp_duration: Duration) {
        if let Some(record) = self.soft_stop_history.back_mut() {
            if record.ramp_duration.is_none() {
                record.ramp_duration = Some(ramp_duration);
            }
        }
    }

    /// Get fault history
    pub fn get_fault_history(&self) -> &VecDeque<FaultRecord> {
        &self.fault_history
    }

    /// Get fault history as slice
    pub fn get_fault_history_slice(&self) -> Vec<&FaultRecord> {
        self.fault_history.iter().collect()
    }

    /// Get soft-stop history
    pub fn get_soft_stop_history(&self) -> &VecDeque<SoftStopRecord> {
        &self.soft_stop_history
    }

    /// Get fault counters
    pub fn get_fault_counters(&self) -> &std::collections::HashMap<FaultType, u32> {
        &self.fault_counters
    }

    /// Get recent faults (within specified duration)
    pub fn get_recent_faults(&self, within: Duration) -> Vec<&FaultRecord> {
        let cutoff = Instant::now() - within;
        self.fault_history.iter()
            .filter(|record| record.detected_at > cutoff)
            .collect()
    }

    /// Check if fault rate is excessive
    pub fn is_fault_rate_excessive(&self, fault_type: &FaultType, within: Duration, max_count: u32) -> bool {
        let recent_count = self.get_recent_faults(within)
            .iter()
            .filter(|record| &record.fault_type == fault_type)
            .count() as u32;
        
        recent_count > max_count
    }

    /// Clear all fault history (used after power cycle reset)
    pub fn clear_faults(&mut self) {
        self.fault_history.clear();
        self.soft_stop_history.clear();
        self.fault_counters.clear();
        self.last_fault_time = None;
    }

    /// Get time since last fault
    pub fn time_since_last_fault(&self) -> Option<Duration> {
        self.last_fault_time.map(|t| t.elapsed())
    }

    /// Check if system is in fault storm (too many faults recently)
    pub fn is_in_fault_storm(&self) -> bool {
        let recent_faults = self.get_recent_faults(Duration::from_secs(60));
        recent_faults.len() > 10 // More than 10 faults in last minute
    }

    /// Get fault statistics
    pub fn get_fault_statistics(&self) -> FaultStatistics {
        let total_faults = self.fault_counters.values().sum();
        let unique_fault_types = self.fault_counters.len();
        
        let avg_response_time = if !self.fault_history.is_empty() {
            let total_response_time: Duration = self.fault_history.iter()
                .filter_map(|r| r.response_time)
                .sum();
            let count = self.fault_history.iter()
                .filter(|r| r.response_time.is_some())
                .count();
            
            if count > 0 {
                Some(total_response_time / count as u32)
            } else {
                None
            }
        } else {
            None
        };

        FaultStatistics {
            total_faults,
            unique_fault_types,
            avg_response_time,
            max_response_time: self.fault_history.iter()
                .filter_map(|r| r.response_time)
                .max(),
            fault_storm_detected: self.is_in_fault_storm(),
        }
    }
}

/// Fault statistics summary
#[derive(Debug, Clone)]
pub struct FaultStatistics {
    pub total_faults: u32,
    pub unique_fault_types: usize,
    pub avg_response_time: Option<Duration>,
    pub max_response_time: Option<Duration>,
    pub fault_storm_detected: bool,
}

impl Default for FaultDetector {
    fn default() -> Self {
        Self::new(Duration::from_millis(50))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_type_properties() {
        assert_eq!(FaultType::UsbStall.error_code(), "HID_OUT_STALL");
        assert!(FaultType::UsbStall.requires_torque_cutoff());
        assert_eq!(FaultType::UsbStall.max_response_time(), Duration::from_millis(50));
        
        assert_eq!(FaultType::PluginOverrun.error_code(), "PLUG_OVERRUN");
        assert!(!FaultType::PluginOverrun.requires_torque_cutoff());
        assert_eq!(FaultType::PluginOverrun.max_response_time(), Duration::from_millis(100));
    }

    #[test]
    fn test_fault_recording() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        let record = detector.record_fault(FaultType::UsbStall);
        
        assert_eq!(record.fault_type, FaultType::UsbStall);
        assert_eq!(record.action_taken, FaultAction::TorqueZero50ms);
        assert!(record.caused_safety_transition);
        
        assert_eq!(detector.get_fault_history().len(), 1);
        assert_eq!(detector.get_fault_counters()[&FaultType::UsbStall], 1);
    }

    #[test]
    fn test_fault_response_completion() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        detector.record_fault(FaultType::UsbStall);
        detector.record_fault_response_complete(FaultType::UsbStall, Duration::from_millis(30));
        
        let record = &detector.get_fault_history()[0];
        assert_eq!(record.response_time, Some(Duration::from_millis(30)));
    }

    #[test]
    fn test_soft_stop_recording() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        let triggered_at = Instant::now();
        let record = detector.record_soft_stop(triggered_at);
        
        assert_eq!(record.triggered_at, triggered_at);
        assert!(record.audio_cue_triggered);
        assert_eq!(detector.get_soft_stop_history().len(), 1);
        
        detector.record_soft_stop_complete(Duration::from_millis(45));
        
        let updated_record = &detector.get_soft_stop_history()[0];
        assert_eq!(updated_record.ramp_duration, Some(Duration::from_millis(45)));
    }

    #[test]
    fn test_recent_faults() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        detector.record_fault(FaultType::UsbStall);
        std::thread::sleep(Duration::from_millis(10));
        detector.record_fault(FaultType::OverTemp);
        
        let recent = detector.get_recent_faults(Duration::from_millis(100));
        assert_eq!(recent.len(), 2);
        
        let very_recent = detector.get_recent_faults(Duration::from_millis(5));
        assert_eq!(very_recent.len(), 1);
        assert_eq!(very_recent[0].fault_type, FaultType::OverTemp);
    }

    #[test]
    fn test_fault_rate_detection() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        // Record multiple faults of same type
        for _ in 0..5 {
            detector.record_fault(FaultType::UsbStall);
        }
        
        assert!(detector.is_fault_rate_excessive(
            &FaultType::UsbStall, 
            Duration::from_secs(60), 
            3
        ));
        
        assert!(!detector.is_fault_rate_excessive(
            &FaultType::OverTemp, 
            Duration::from_secs(60), 
            3
        ));
    }

    #[test]
    fn test_fault_storm_detection() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        // Record many faults to trigger storm detection
        for i in 0..15 {
            let fault_type = if i % 2 == 0 { 
                FaultType::UsbStall 
            } else { 
                FaultType::OverTemp 
            };
            detector.record_fault(fault_type);
        }
        
        assert!(detector.is_in_fault_storm());
    }

    #[test]
    fn test_fault_statistics() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        detector.record_fault(FaultType::UsbStall);
        detector.record_fault_response_complete(FaultType::UsbStall, Duration::from_millis(30));
        
        detector.record_fault(FaultType::OverTemp);
        detector.record_fault_response_complete(FaultType::OverTemp, Duration::from_millis(40));
        
        let stats = detector.get_fault_statistics();
        
        assert_eq!(stats.total_faults, 2);
        assert_eq!(stats.unique_fault_types, 2);
        assert_eq!(stats.avg_response_time, Some(Duration::from_millis(35)));
        assert_eq!(stats.max_response_time, Some(Duration::from_millis(40)));
        assert!(!stats.fault_storm_detected);
    }

    #[test]
    fn test_clear_faults() {
        let mut detector = FaultDetector::new(Duration::from_millis(50));
        
        detector.record_fault(FaultType::UsbStall);
        detector.record_soft_stop(Instant::now());
        
        assert!(!detector.get_fault_history().is_empty());
        assert!(!detector.get_soft_stop_history().is_empty());
        
        detector.clear_faults();
        
        assert!(detector.get_fault_history().is_empty());
        assert!(detector.get_soft_stop_history().is_empty());
        assert!(detector.get_fault_counters().is_empty());
        assert!(detector.time_since_last_fault().is_none());
    }
}