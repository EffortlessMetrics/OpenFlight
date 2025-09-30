//! Physical interlock system for force feedback safety
//!
//! Implements challenge/response system with rolling tokens to prevent
//! remote unlock of high-torque mode. Requires physical device interaction.

use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Physical interlock challenge sent to device
#[derive(Debug, Clone, PartialEq)]
pub struct InterlockChallenge {
    /// Unique challenge ID
    pub challenge_id: u64,
    /// Rolling token for this challenge
    pub token: u32,
    /// Blink pattern for device to display
    pub blink_pattern: BlinkPattern,
    /// Challenge expiration time
    pub expires_at: Instant,
    /// Required button combination
    pub required_buttons: ButtonCombination,
}

/// Blink pattern for device visual feedback
#[derive(Debug, Clone, PartialEq)]
pub struct BlinkPattern {
    /// Sequence of on/off durations in milliseconds
    pub sequence: Vec<u16>,
    /// Number of times to repeat the pattern
    pub repeat_count: u8,
    /// LED color if device supports it
    pub color: LedColor,
}

/// LED color options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LedColor {
    Red,
    Green,
    Blue,
    Yellow,
    White,
}

/// Required button combination for interlock
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonCombination {
    /// Primary button (e.g., trigger)
    pub primary: ButtonId,
    /// Secondary button (e.g., thumb button)
    pub secondary: ButtonId,
    /// Minimum hold duration in milliseconds
    pub hold_duration_ms: u16,
}

/// Button identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonId {
    Trigger,
    ThumbButton,
    BaseButton1,
    BaseButton2,
    HatUp,
    HatDown,
    HatLeft,
    HatRight,
}

/// Response from device to interlock challenge
#[derive(Debug, Clone, PartialEq)]
pub struct InterlockResponse {
    /// Challenge ID this response is for
    pub challenge_id: u64,
    /// Token echoed back from device
    pub echoed_token: u32,
    /// Buttons that were pressed
    pub buttons_pressed: Vec<ButtonId>,
    /// Duration buttons were held in milliseconds
    pub hold_duration_ms: u16,
    /// Timestamp when response was generated
    pub response_timestamp: Instant,
}

/// Interlock system state
#[derive(Debug)]
pub struct InterlockSystem {
    /// Whether interlock is required for high torque
    required: bool,
    /// Current active challenge
    active_challenge: Option<InterlockChallenge>,
    /// Challenge history for replay attack prevention
    challenge_history: HashMap<u64, Instant>,
    /// Whether interlock is currently satisfied
    satisfied: bool,
    /// When interlock was last satisfied
    last_satisfied: Option<Instant>,
    /// Rolling token counter
    token_counter: u32,
    /// Challenge ID counter
    challenge_counter: u64,
    /// Maximum challenge lifetime
    challenge_timeout: Duration,
    /// Maximum time interlock remains satisfied
    satisfaction_timeout: Duration,
}

/// Interlock system errors
#[derive(Debug, thiserror::Error)]
pub enum InterlockError {
    #[error("Challenge expired")]
    ChallengeExpired,
    #[error("Invalid token: expected {expected}, got {actual}")]
    InvalidToken { expected: u32, actual: u32 },
    #[error("Wrong button combination: expected {expected:?}, got {actual:?}")]
    WrongButtonCombination { expected: ButtonCombination, actual: Vec<ButtonId> },
    #[error("Insufficient hold duration: expected {expected}ms, got {actual}ms")]
    InsufficientHoldDuration { expected: u16, actual: u16 },
    #[error("No active challenge")]
    NoActiveChallenge,
    #[error("Challenge already used (replay attack prevention)")]
    ChallengeAlreadyUsed,
    #[error("Response too late: challenge expired {expired_ago:?} ago")]
    ResponseTooLate { expired_ago: Duration },
}

pub type InterlockResult<T> = std::result::Result<T, InterlockError>;

impl InterlockSystem {
    /// Create new interlock system
    pub fn new(required: bool) -> Self {
        Self {
            required,
            active_challenge: None,
            challenge_history: HashMap::new(),
            satisfied: false,
            last_satisfied: None,
            token_counter: 1,
            challenge_counter: 1,
            challenge_timeout: Duration::from_secs(30), // 30 second challenge timeout
            satisfaction_timeout: Duration::from_secs(300), // 5 minute satisfaction timeout
        }
    }

    /// Check if interlock is currently satisfied
    pub fn is_satisfied(&self) -> bool {
        if !self.required {
            return true;
        }

        if !self.satisfied {
            return false;
        }

        // Check if satisfaction has expired
        if let Some(last_satisfied) = self.last_satisfied {
            if last_satisfied.elapsed() > self.satisfaction_timeout {
                return false;
            }
        }

        true
    }

    /// Generate new interlock challenge
    pub fn generate_challenge(&mut self) -> InterlockResult<InterlockChallenge> {
        // Generate rolling token
        self.token_counter = self.token_counter.wrapping_add(1);
        if self.token_counter == 0 {
            self.token_counter = 1; // Avoid zero token
        }

        // Generate challenge ID
        self.challenge_counter = self.challenge_counter.wrapping_add(1);

        // Create blink pattern based on token
        let blink_pattern = self.generate_blink_pattern(self.token_counter);

        // Standard button combination: trigger + thumb button for 2 seconds
        let button_combination = ButtonCombination {
            primary: ButtonId::Trigger,
            secondary: ButtonId::ThumbButton,
            hold_duration_ms: 2000,
        };

        let challenge = InterlockChallenge {
            challenge_id: self.challenge_counter,
            token: self.token_counter,
            blink_pattern,
            expires_at: Instant::now() + self.challenge_timeout,
            required_buttons: button_combination,
        };

        // Store active challenge
        self.active_challenge = Some(challenge.clone());

        // Add to history for replay prevention
        self.challenge_history.insert(challenge.challenge_id, Instant::now());

        // Clean old history entries (keep last 100)
        if self.challenge_history.len() > 100 {
            let cutoff = Instant::now() - Duration::from_secs(3600); // 1 hour
            self.challenge_history.retain(|_, timestamp| *timestamp > cutoff);
        }

        Ok(challenge)
    }

    /// Validate interlock response from device
    pub fn validate_response(&mut self, response: InterlockResponse) -> InterlockResult<bool> {
        // Check if we have an active challenge
        let challenge = self.active_challenge.as_ref()
            .ok_or(InterlockError::NoActiveChallenge)?;

        // Check challenge ID matches
        if response.challenge_id != challenge.challenge_id {
            return Err(InterlockError::NoActiveChallenge);
        }

        // Check if challenge has expired
        if Instant::now() > challenge.expires_at {
            let expired_ago = Instant::now().duration_since(challenge.expires_at);
            return Err(InterlockError::ResponseTooLate { expired_ago });
        }

        // Check token matches
        if response.echoed_token != challenge.token {
            return Err(InterlockError::InvalidToken {
                expected: challenge.token,
                actual: response.echoed_token,
            });
        }

        // Check button combination
        let required_buttons = vec![
            challenge.required_buttons.primary,
            challenge.required_buttons.secondary,
        ];
        
        if !self.buttons_match(&required_buttons, &response.buttons_pressed) {
            return Err(InterlockError::WrongButtonCombination {
                expected: challenge.required_buttons.clone(),
                actual: response.buttons_pressed,
            });
        }

        // Check hold duration
        if response.hold_duration_ms < challenge.required_buttons.hold_duration_ms {
            return Err(InterlockError::InsufficientHoldDuration {
                expected: challenge.required_buttons.hold_duration_ms,
                actual: response.hold_duration_ms,
            });
        }

        // All checks passed - satisfy interlock
        self.satisfied = true;
        self.last_satisfied = Some(Instant::now());
        self.active_challenge = None;

        Ok(true)
    }

    /// Reset interlock satisfaction (called on power cycle or user disable)
    pub fn reset(&mut self) {
        self.satisfied = false;
        self.last_satisfied = None;
        self.active_challenge = None;
    }

    /// Get current active challenge
    pub fn active_challenge(&self) -> Option<&InterlockChallenge> {
        self.active_challenge.as_ref()
    }

    /// Check if interlock is required
    pub fn is_required(&self) -> bool {
        self.required
    }

    /// Set whether interlock is required
    pub fn set_required(&mut self, required: bool) {
        self.required = required;
        if !required {
            self.satisfied = true; // Auto-satisfy if not required
        }
    }

    /// Generate blink pattern based on token
    fn generate_blink_pattern(&self, token: u32) -> BlinkPattern {
        // Generate pattern based on token bits
        let mut sequence = Vec::new();
        
        // Use lower 8 bits to generate pattern
        let pattern_bits = (token & 0xFF) as u8;
        
        for i in 0..4 {
            let bit = (pattern_bits >> (i * 2)) & 0x03;
            match bit {
                0 => {
                    sequence.push(200); // Short on
                    sequence.push(200); // Short off
                }
                1 => {
                    sequence.push(500); // Long on
                    sequence.push(200); // Short off
                }
                2 => {
                    sequence.push(200); // Short on
                    sequence.push(500); // Long off
                }
                3 => {
                    sequence.push(500); // Long on
                    sequence.push(500); // Long off
                }
                _ => unreachable!(),
            }
        }

        BlinkPattern {
            sequence,
            repeat_count: 3,
            color: LedColor::Yellow,
        }
    }

    /// Check if button combinations match
    fn buttons_match(&self, required: &[ButtonId], pressed: &[ButtonId]) -> bool {
        if required.len() != pressed.len() {
            return false;
        }

        // Check all required buttons are pressed
        for &required_button in required {
            if !pressed.contains(&required_button) {
                return false;
            }
        }

        // Check no extra buttons are pressed
        for &pressed_button in pressed {
            if !required.contains(&pressed_button) {
                return false;
            }
        }

        true
    }
}

impl Default for InterlockSystem {
    fn default() -> Self {
        Self::new(true) // Default to requiring interlock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interlock_not_required() {
        let system = InterlockSystem::new(false);
        assert!(system.is_satisfied()); // Should be satisfied when not required
    }

    #[test]
    fn test_challenge_generation() {
        let mut system = InterlockSystem::new(true);
        
        let challenge1 = system.generate_challenge().unwrap();
        let challenge2 = system.generate_challenge().unwrap();
        
        // Challenges should be unique
        assert_ne!(challenge1.challenge_id, challenge2.challenge_id);
        assert_ne!(challenge1.token, challenge2.token);
    }

    #[test]
    fn test_valid_response() {
        let mut system = InterlockSystem::new(true);
        
        let challenge = system.generate_challenge().unwrap();
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token,
            buttons_pressed: vec![ButtonId::Trigger, ButtonId::ThumbButton],
            hold_duration_ms: 2000,
            response_timestamp: Instant::now(),
        };
        
        assert!(system.validate_response(response).unwrap());
        assert!(system.is_satisfied());
    }

    #[test]
    fn test_invalid_token() {
        let mut system = InterlockSystem::new(true);
        
        let challenge = system.generate_challenge().unwrap();
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token + 1, // Wrong token
            buttons_pressed: vec![ButtonId::Trigger, ButtonId::ThumbButton],
            hold_duration_ms: 2000,
            response_timestamp: Instant::now(),
        };
        
        assert!(matches!(
            system.validate_response(response),
            Err(InterlockError::InvalidToken { .. })
        ));
        assert!(!system.is_satisfied());
    }

    #[test]
    fn test_wrong_buttons() {
        let mut system = InterlockSystem::new(true);
        
        let challenge = system.generate_challenge().unwrap();
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token,
            buttons_pressed: vec![ButtonId::BaseButton1], // Wrong buttons
            hold_duration_ms: 2000,
            response_timestamp: Instant::now(),
        };
        
        assert!(matches!(
            system.validate_response(response),
            Err(InterlockError::WrongButtonCombination { .. })
        ));
        assert!(!system.is_satisfied());
    }

    #[test]
    fn test_insufficient_hold_duration() {
        let mut system = InterlockSystem::new(true);
        
        let challenge = system.generate_challenge().unwrap();
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token,
            buttons_pressed: vec![ButtonId::Trigger, ButtonId::ThumbButton],
            hold_duration_ms: 1000, // Too short
            response_timestamp: Instant::now(),
        };
        
        assert!(matches!(
            system.validate_response(response),
            Err(InterlockError::InsufficientHoldDuration { .. })
        ));
        assert!(!system.is_satisfied());
    }

    #[test]
    fn test_challenge_expiration() {
        let mut system = InterlockSystem::new(true);
        system.challenge_timeout = Duration::from_millis(1); // Very short timeout
        
        let challenge = system.generate_challenge().unwrap();
        
        // Wait for challenge to expire
        std::thread::sleep(Duration::from_millis(10));
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token,
            buttons_pressed: vec![ButtonId::Trigger, ButtonId::ThumbButton],
            hold_duration_ms: 2000,
            response_timestamp: Instant::now(),
        };
        
        assert!(matches!(
            system.validate_response(response),
            Err(InterlockError::ResponseTooLate { .. })
        ));
    }

    #[test]
    fn test_satisfaction_timeout() {
        let mut system = InterlockSystem::new(true);
        system.satisfaction_timeout = Duration::from_millis(1); // Very short timeout
        
        let challenge = system.generate_challenge().unwrap();
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token,
            buttons_pressed: vec![ButtonId::Trigger, ButtonId::ThumbButton],
            hold_duration_ms: 2000,
            response_timestamp: Instant::now(),
        };
        
        assert!(system.validate_response(response).unwrap());
        assert!(system.is_satisfied());
        
        // Wait for satisfaction to expire
        std::thread::sleep(Duration::from_millis(10));
        assert!(!system.is_satisfied());
    }

    #[test]
    fn test_reset() {
        let mut system = InterlockSystem::new(true);
        
        let challenge = system.generate_challenge().unwrap();
        
        let response = InterlockResponse {
            challenge_id: challenge.challenge_id,
            echoed_token: challenge.token,
            buttons_pressed: vec![ButtonId::Trigger, ButtonId::ThumbButton],
            hold_duration_ms: 2000,
            response_timestamp: Instant::now(),
        };
        
        assert!(system.validate_response(response).unwrap());
        assert!(system.is_satisfied());
        
        system.reset();
        assert!(!system.is_satisfied());
        assert!(system.active_challenge().is_none());
    }

    #[test]
    fn test_blink_pattern_generation() {
        let system = InterlockSystem::new(true);
        
        let pattern1 = system.generate_blink_pattern(0x12345678);
        let pattern2 = system.generate_blink_pattern(0x87654321);
        
        // Different tokens should generate different patterns
        assert_ne!(pattern1.sequence, pattern2.sequence);
        
        // Pattern should have reasonable length
        assert!(!pattern1.sequence.is_empty());
        assert!(pattern1.sequence.len() <= 16); // 4 bits * 2 values per bit
    }
}