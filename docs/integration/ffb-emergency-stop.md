# FFB Emergency Stop UI Integration Guide

## Overview

This guide explains how to integrate the FFB emergency stop functionality with a UI "big red button". The emergency stop is a critical safety feature that immediately disables all force feedback output, ramping torque to zero within 50ms.

**Validates: Requirements FFB-SAFETY-01.14, FFB-SAFETY-04**

## Quick Start

```rust
use flight_ffb::{FfbEngine, FfbConfig, EmergencyStopReason};

// Create FFB engine
let mut engine = FfbEngine::new(FfbConfig::default())?;

// When user clicks the emergency stop button:
engine.emergency_stop(EmergencyStopReason::UiButton)?;

// Check if emergency stop is active (for UI state):
if engine.is_emergency_stop_active() {
    // Show "Emergency Stop Active" indicator
}

// When user clicks "Clear Emergency Stop" button:
engine.clear_emergency_stop()?;
```

## API Reference

### `EmergencyStopReason` Enum

The reason for triggering an emergency stop:

```rust
pub enum EmergencyStopReason {
    /// User pressed UI emergency stop button (big red button)
    UiButton,
    /// Hardware emergency stop button pressed (physical button on device)
    HardwareButton,
    /// Programmatic emergency stop (e.g., from external system or watchdog)
    Programmatic,
}
```

### `FfbEngine::emergency_stop(reason: EmergencyStopReason)`

Triggers an immediate emergency stop. This method:

1. **Bypasses all normal processing** - Goes directly to fault handling
2. **Captures current torque** - Records the torque value at the moment of trigger
3. **Initiates 50ms ramp-down** - Smoothly reduces torque to zero within 50ms
4. **Transitions to Faulted state** - Sets `safety_state` to `SafetyState::Faulted`
5. **Records in blackbox** - Logs the event for diagnostics
6. **Triggers audio cue** - Plays fault warning sound (if audio enabled)

**Returns:** `Result<()>` - Always succeeds unless blackbox recording fails

**Example:**
```rust
// In your UI button click handler:
fn on_emergency_stop_clicked(engine: &mut FfbEngine) {
    if let Err(e) = engine.emergency_stop(EmergencyStopReason::UiButton) {
        log::error!("Failed to trigger emergency stop: {}", e);
        // Note: This should rarely fail - the emergency stop is designed
        // to work even in degraded conditions
    }
}
```

### `FfbEngine::clear_emergency_stop()`

Clears the emergency stop state and returns to `SafeTorque` mode.

**Important:** This only clears emergency stops triggered by `UiButton` or `HardwareButton`. Hardware-critical faults (over-temp, over-current) require a power cycle.

**Returns:** `Result<()>`

**Example:**
```rust
// In your "Clear Emergency Stop" button click handler:
fn on_clear_emergency_stop_clicked(engine: &mut FfbEngine) {
    if let Err(e) = engine.clear_emergency_stop() {
        log::error!("Failed to clear emergency stop: {}", e);
    }
}
```

### `FfbEngine::is_emergency_stop_active()`

Returns `true` if the system is in emergency stop (faulted) state.

**Use this to:**
- Update UI button state (enabled/disabled)
- Show/hide emergency stop indicator
- Prevent other FFB operations while stopped

**Example:**
```rust
// In your UI update loop:
fn update_ui(engine: &FfbEngine, ui: &mut Ui) {
    if engine.is_emergency_stop_active() {
        ui.show_emergency_stop_indicator();
        ui.disable_ffb_controls();
        ui.enable_clear_button();
    } else {
        ui.hide_emergency_stop_indicator();
        ui.enable_ffb_controls();
        ui.disable_clear_button();
    }
}
```

## UI Design Recommendations

### Emergency Stop Button

The emergency stop button should be:

1. **Highly visible** - Use a large, red button that stands out
2. **Always accessible** - Never hidden or disabled
3. **Single-click activation** - No confirmation dialog (speed is critical)
4. **Clear labeling** - "EMERGENCY STOP" or "E-STOP" with stop icon

```
┌─────────────────────────────────────┐
│                                     │
│    ┌───────────────────────────┐    │
│    │                           │    │
│    │     🛑 EMERGENCY STOP     │    │
│    │                           │    │
│    └───────────────────────────┘    │
│                                     │
│    Status: ● Active / ○ Ready       │
│                                     │
│    [Clear Emergency Stop]           │
│                                     │
└─────────────────────────────────────┘
```

### State Indicators

Show clear visual feedback for the current state:

| State | Indicator | Button State |
|-------|-----------|--------------|
| Normal (SafeTorque/HighTorque) | Green "Ready" | E-Stop enabled |
| Emergency Stop Active | Red "STOPPED" | E-Stop disabled, Clear enabled |
| Clearing | Yellow "Clearing..." | Both disabled |

### Keyboard Shortcut

Consider binding a keyboard shortcut for quick access:

```rust
// Example: Bind Escape key to emergency stop
fn handle_key_press(key: Key, engine: &mut FfbEngine) {
    if key == Key::Escape {
        let _ = engine.emergency_stop(EmergencyStopReason::UiButton);
    }
}
```

## Complete Integration Example

```rust
use flight_ffb::{FfbEngine, FfbConfig, EmergencyStopReason, SafetyState};

/// UI state for emergency stop panel
pub struct EmergencyStopPanel {
    engine: FfbEngine,
}

impl EmergencyStopPanel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = FfbConfig::default();
        let engine = FfbEngine::new(config)?;
        Ok(Self { engine })
    }

    /// Called when user clicks the emergency stop button
    pub fn on_emergency_stop_clicked(&mut self) {
        match self.engine.emergency_stop(EmergencyStopReason::UiButton) {
            Ok(()) => {
                log::warn!("Emergency stop activated by user");
                // UI will update on next frame via is_emergency_stop_active()
            }
            Err(e) => {
                log::error!("Emergency stop failed: {}", e);
                // Show error to user - this is a critical failure
            }
        }
    }

    /// Called when user clicks the clear button
    pub fn on_clear_clicked(&mut self) {
        match self.engine.clear_emergency_stop() {
            Ok(()) => {
                log::info!("Emergency stop cleared by user");
            }
            Err(e) => {
                log::error!("Failed to clear emergency stop: {}", e);
            }
        }
    }

    /// Get current state for UI rendering
    pub fn get_ui_state(&self) -> EmergencyStopUiState {
        EmergencyStopUiState {
            is_active: self.engine.is_emergency_stop_active(),
            safety_state: self.engine.safety_state(),
            can_clear: self.engine.is_emergency_stop_active() 
                && !self.engine.is_fault_hardware_critical(),
        }
    }
}

/// UI state for rendering
pub struct EmergencyStopUiState {
    pub is_active: bool,
    pub safety_state: SafetyState,
    pub can_clear: bool,
}
```

## Thread Safety Considerations

The `FfbEngine` is **not thread-safe** by default. If your UI runs on a different thread than the FFB loop:

1. **Use a channel** to send commands to the FFB thread:

```rust
use std::sync::mpsc::{channel, Sender, Receiver};

enum FfbCommand {
    EmergencyStop,
    ClearEmergencyStop,
}

// UI thread sends commands
fn on_button_click(tx: &Sender<FfbCommand>) {
    tx.send(FfbCommand::EmergencyStop).unwrap();
}

// FFB thread processes commands
fn ffb_loop(rx: Receiver<FfbCommand>, engine: &mut FfbEngine) {
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            FfbCommand::EmergencyStop => {
                let _ = engine.emergency_stop(EmergencyStopReason::UiButton);
            }
            FfbCommand::ClearEmergencyStop => {
                let _ = engine.clear_emergency_stop();
            }
        }
    }
}
```

2. **Use atomic state** for UI queries:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static EMERGENCY_STOP_ACTIVE: AtomicBool = AtomicBool::new(false);

// FFB thread updates state
fn update_state(engine: &FfbEngine) {
    EMERGENCY_STOP_ACTIVE.store(
        engine.is_emergency_stop_active(),
        Ordering::SeqCst
    );
}

// UI thread reads state
fn is_stopped() -> bool {
    EMERGENCY_STOP_ACTIVE.load(Ordering::SeqCst)
}
```

## Testing

The emergency stop functionality is thoroughly tested. See:
- `crates/flight-ffb/src/tests.rs` - Unit tests for emergency stop
- `crates/flight-ffb/src/tests/fault_detection_blackbox_tests.rs` - Integration tests

To run the tests:
```bash
cargo test -p flight-ffb emergency_stop
```

## Related Documentation

- [FFB Safety Design](../adr/009-safety-interlock-design.md) - Safety architecture decisions
- [XInput Integration](./xinput-integration-guide.md) - XInput rumble fallback
- [XInput Limitations](./xinput-limitations.md) - XInput vs DirectInput FFB

## Requirements Traceability

| Requirement | Implementation |
|-------------|----------------|
| FFB-SAFETY-01.14 | `FfbEngine::emergency_stop()` with UI and hardware button support |
| FFB-SAFETY-04 | Emergency stop bypasses all processing, 50ms ramp-down |
| FFB-SAFETY-01.10 | `FfbEngine::clear_emergency_stop()` for transient fault clearing |
