//! Audio cue system for force feedback safety events
//!
//! Provides cross-platform audio alerts for fault conditions and safety events.

use std::time::{Duration, Instant};
use thiserror::Error;

/// Audio cue types for different events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCueType {
    /// Fault detected - urgent warning
    FaultWarning,
    /// Soft-stop initiated
    SoftStop,
    /// High torque mode enabled
    HighTorqueEnabled,
    /// High torque mode disabled
    HighTorqueDisabled,
    /// System error
    SystemError,
}

impl AudioCueType {
    /// Get the audio pattern for this cue type
    pub fn pattern(&self) -> AudioPattern {
        match self {
            AudioCueType::FaultWarning => AudioPattern {
                frequency_hz: 800.0,
                duration: Duration::from_millis(200),
                repeat_count: 3,
                repeat_interval: Duration::from_millis(100),
            },
            AudioCueType::SoftStop => AudioPattern {
                frequency_hz: 600.0,
                duration: Duration::from_millis(500),
                repeat_count: 1,
                repeat_interval: Duration::ZERO,
            },
            AudioCueType::HighTorqueEnabled => AudioPattern {
                frequency_hz: 1000.0,
                duration: Duration::from_millis(150),
                repeat_count: 2,
                repeat_interval: Duration::from_millis(50),
            },
            AudioCueType::HighTorqueDisabled => AudioPattern {
                frequency_hz: 400.0,
                duration: Duration::from_millis(300),
                repeat_count: 1,
                repeat_interval: Duration::ZERO,
            },
            AudioCueType::SystemError => AudioPattern {
                frequency_hz: 1200.0,
                duration: Duration::from_millis(100),
                repeat_count: 5,
                repeat_interval: Duration::from_millis(80),
            },
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            AudioCueType::FaultWarning => "Fault warning",
            AudioCueType::SoftStop => "Soft-stop initiated",
            AudioCueType::HighTorqueEnabled => "High torque enabled",
            AudioCueType::HighTorqueDisabled => "High torque disabled",
            AudioCueType::SystemError => "System error",
        }
    }
}

/// Audio pattern definition
#[derive(Debug, Clone)]
pub struct AudioPattern {
    /// Frequency in Hz
    pub frequency_hz: f32,
    /// Duration of each beep
    pub duration: Duration,
    /// Number of times to repeat
    pub repeat_count: u32,
    /// Interval between repeats
    pub repeat_interval: Duration,
}

/// Audio cue configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Whether audio cues are enabled
    pub enabled: bool,
    /// Master volume (0.0 to 1.0)
    pub volume: f32,
    /// Minimum interval between cues to prevent spam
    pub min_interval: Duration,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.7,
            min_interval: Duration::from_millis(100),
        }
    }
}

/// Audio cue playback state
#[derive(Debug, Clone)]
struct AudioPlaybackState {
    cue_type: AudioCueType,
    pattern: AudioPattern,
    started_at: Instant,
    current_repeat: u32,
    last_beep_start: Option<Instant>,
}

/// Audio cue system
#[derive(Debug)]
pub struct AudioCueSystem {
    config: AudioConfig,
    current_playback: Option<AudioPlaybackState>,
    last_cue_time: Option<Instant>,
}

/// Audio system errors
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio system not available")]
    SystemNotAvailable,
    #[error("Audio cue rate limited")]
    RateLimited,
    #[error("Invalid volume: {volume} (must be 0.0-1.0)")]
    InvalidVolume { volume: f32 },
    #[error("Audio playback failed: {message}")]
    PlaybackFailed { message: String },
}

pub type AudioResult<T> = std::result::Result<T, AudioError>;

impl AudioCueSystem {
    /// Create new audio cue system
    pub fn new(config: AudioConfig) -> AudioResult<Self> {
        if !(0.0..=1.0).contains(&config.volume) {
            return Err(AudioError::InvalidVolume { volume: config.volume });
        }

        Ok(Self {
            config,
            current_playback: None,
            last_cue_time: None,
        })
    }

    /// Trigger an audio cue
    pub fn trigger_cue(&mut self, cue_type: AudioCueType) -> AudioResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let now = Instant::now();

        // Check rate limiting
        if let Some(last_time) = self.last_cue_time {
            if now.duration_since(last_time) < self.config.min_interval {
                return Err(AudioError::RateLimited);
            }
        }

        // Stop any current playback
        self.stop_current_playback();

        // Start new playback
        let pattern = cue_type.pattern();
        self.current_playback = Some(AudioPlaybackState {
            cue_type,
            pattern: pattern.clone(),
            started_at: now,
            current_repeat: 0,
            last_beep_start: None,
        });

        self.last_cue_time = Some(now);

        // Trigger the first beep
        self.play_beep(&pattern)?;

        Ok(())
    }

    /// Update audio system (should be called regularly)
    pub fn update(&mut self) -> AudioResult<()> {
        let mut should_play_beep = false;
        let mut pattern_to_play = None;
        let mut playback_complete = false;
        
        if let Some(playback) = &mut self.current_playback {
            let now = Instant::now();
            
            // Check if we need to start the next repeat
            if playback.current_repeat < playback.pattern.repeat_count {
                let should_start_next = if let Some(last_start) = playback.last_beep_start {
                    let time_since_last = now.duration_since(last_start);
                    time_since_last >= playback.pattern.duration + playback.pattern.repeat_interval
                } else {
                    // First beep already started in trigger_cue
                    playback.last_beep_start = Some(playback.started_at);
                    playback.current_repeat = 1;
                    false
                };

                if should_start_next {
                    should_play_beep = true;
                    pattern_to_play = Some(playback.pattern.clone());
                    playback.last_beep_start = Some(now);
                    playback.current_repeat += 1;
                }
            } else {
                // Playback complete
                playback_complete = true;
            }
        }

        // Play beep outside of the borrow
        if should_play_beep {
            if let Some(pattern) = pattern_to_play {
                self.play_beep(&pattern)?;
            }
        }

        // Complete playback outside of the borrow
        if playback_complete {
            self.current_playback = None;
        }

        Ok(())
    }

    /// Check if audio cue is currently playing
    pub fn is_playing(&self) -> bool {
        self.current_playback.is_some()
    }

    /// Get current playback info
    pub fn current_cue(&self) -> Option<AudioCueType> {
        self.current_playback.as_ref().map(|p| p.cue_type)
    }

    /// Stop current playback
    pub fn stop_current_playback(&mut self) {
        if self.current_playback.is_some() {
            // In a real implementation, this would stop the audio hardware
            self.current_playback = None;
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: AudioConfig) -> AudioResult<()> {
        if !(0.0..=1.0).contains(&config.volume) {
            return Err(AudioError::InvalidVolume { volume: config.volume });
        }
        
        self.config = config;
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &AudioConfig {
        &self.config
    }

    /// Enable/disable audio cues
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.stop_current_playback();
        }
    }

    /// Set volume (0.0 to 1.0)
    pub fn set_volume(&mut self, volume: f32) -> AudioResult<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(AudioError::InvalidVolume { volume });
        }
        self.config.volume = volume;
        Ok(())
    }

    /// Platform-specific beep implementation
    fn play_beep(&self, pattern: &AudioPattern) -> AudioResult<()> {
        // This is a stub implementation
        // In a real system, this would:
        // - On Windows: Use Beep() API or DirectSound
        // - On Linux: Use ALSA or PulseAudio
        // - Cross-platform: Use a crate like rodio or cpal
        
        tracing::info!(
            "Audio cue: {}Hz for {:?} (volume: {:.1})",
            pattern.frequency_hz,
            pattern.duration,
            self.config.volume
        );

        // For now, just log the beep
        // TODO: Implement actual audio output
        
        Ok(())
    }
}

impl Default for AudioCueSystem {
    fn default() -> Self {
        Self::new(AudioConfig::default()).expect("Default audio config should be valid")
    }
}

/// Cross-platform audio utilities
pub mod platform {
    use super::*;

    /// Initialize platform-specific audio system
    pub fn init_audio_system() -> AudioResult<()> {
        // Platform-specific initialization
        #[cfg(target_os = "windows")]
        {
            // Windows-specific audio initialization
            // Could use winapi::um::utilapiset::Beep or DirectSound
            Ok(())
        }

        #[cfg(target_os = "linux")]
        {
            // Linux-specific audio initialization
            // Could use ALSA or PulseAudio
            Ok(())
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(AudioError::SystemNotAvailable)
        }
    }

    /// Play system beep (fallback)
    pub fn system_beep(frequency_hz: f32, duration: Duration) -> AudioResult<()> {
        #[cfg(target_os = "windows")]
        {
            // Use Windows Beep API
            use std::os::raw::c_ulong;
            extern "system" {
                fn Beep(frequency: c_ulong, duration: c_ulong) -> i32;
            }
            
            unsafe {
                let result = Beep(
                    frequency_hz as c_ulong,
                    duration.as_millis() as c_ulong
                );
                if result == 0 {
                    Err(AudioError::PlaybackFailed {
                        message: "Windows Beep API failed".to_string()
                    })
                } else {
                    Ok(())
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Use console beep or ALSA
            // For now, just use the console bell
            print!("\x07");
            Ok(())
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(AudioError::SystemNotAvailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_audio_cue_patterns() {
        let fault_pattern = AudioCueType::FaultWarning.pattern();
        assert_eq!(fault_pattern.frequency_hz, 800.0);
        assert_eq!(fault_pattern.repeat_count, 3);

        let soft_stop_pattern = AudioCueType::SoftStop.pattern();
        assert_eq!(soft_stop_pattern.frequency_hz, 600.0);
        assert_eq!(soft_stop_pattern.repeat_count, 1);
    }

    #[test]
    fn test_audio_config_validation() {
        let valid_config = AudioConfig {
            enabled: true,
            volume: 0.5,
            min_interval: Duration::from_millis(100),
        };
        
        let system = AudioCueSystem::new(valid_config);
        assert!(system.is_ok());

        let invalid_config = AudioConfig {
            enabled: true,
            volume: 1.5, // Invalid volume
            min_interval: Duration::from_millis(100),
        };
        
        let system = AudioCueSystem::new(invalid_config);
        assert!(matches!(system, Err(AudioError::InvalidVolume { .. })));
    }

    #[test]
    fn test_cue_triggering() {
        let mut system = AudioCueSystem::default();
        
        // Trigger a cue
        system.trigger_cue(AudioCueType::FaultWarning).unwrap();
        assert!(system.is_playing());
        assert_eq!(system.current_cue(), Some(AudioCueType::FaultWarning));
        
        // Stop playback
        system.stop_current_playback();
        assert!(!system.is_playing());
        assert_eq!(system.current_cue(), None);
    }

    #[test]
    fn test_rate_limiting() {
        let config = AudioConfig {
            enabled: true,
            volume: 0.5,
            min_interval: Duration::from_millis(100),
        };
        
        let mut system = AudioCueSystem::new(config).unwrap();
        
        // First cue should work
        system.trigger_cue(AudioCueType::FaultWarning).unwrap();
        
        // Immediate second cue should be rate limited
        let result = system.trigger_cue(AudioCueType::SoftStop);
        assert!(matches!(result, Err(AudioError::RateLimited)));
    }

    #[test]
    fn test_disabled_system() {
        let config = AudioConfig {
            enabled: false,
            volume: 0.5,
            min_interval: Duration::from_millis(100),
        };
        
        let mut system = AudioCueSystem::new(config).unwrap();
        
        // Should succeed but not actually play
        system.trigger_cue(AudioCueType::FaultWarning).unwrap();
        assert!(!system.is_playing());
    }

    #[test]
    fn test_volume_setting() {
        let mut system = AudioCueSystem::default();
        
        // Valid volume
        system.set_volume(0.3).unwrap();
        assert_eq!(system.get_config().volume, 0.3);
        
        // Invalid volume
        let result = system.set_volume(1.5);
        assert!(matches!(result, Err(AudioError::InvalidVolume { .. })));
    }

    #[test]
    fn test_playback_update() {
        let mut system = AudioCueSystem::default();
        
        // Trigger a multi-repeat cue
        system.trigger_cue(AudioCueType::FaultWarning).unwrap();
        assert!(system.is_playing());
        
        // Update should handle repeat logic
        for _ in 0..10 {
            system.update().unwrap();
            thread::sleep(Duration::from_millis(50));
        }
        
        // Eventually should complete
        // Note: In a real test, we'd need to wait for the full pattern duration
    }

    #[test]
    fn test_cue_interruption() {
        let mut system = AudioCueSystem::default();
        
        // Start first cue
        system.trigger_cue(AudioCueType::SoftStop).unwrap();
        assert_eq!(system.current_cue(), Some(AudioCueType::SoftStop));
        
        // Wait to avoid rate limiting
        thread::sleep(Duration::from_millis(150));
        
        // Start second cue (should interrupt first)
        system.trigger_cue(AudioCueType::FaultWarning).unwrap();
        assert_eq!(system.current_cue(), Some(AudioCueType::FaultWarning));
    }
}