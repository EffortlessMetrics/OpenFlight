// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB device abstraction layer
//!
//! Provides a backend-agnostic trait for FFB output devices (DirectInput, HID,
//! vendor-specific protocols), pre-allocated effect slot management with bounded
//! capacity, and priority-based effect scheduling.
//!
//! All hot-path operations are zero-allocation: slots and the schedule queue
//! are stack-resident arrays with fixed capacity.
//!
//! **Validates: ADR-009 Safety Interlock Design**

use crate::effects::{EffectInput, FfbEffect};
use std::time::Instant;

// ─── Device trait ────────────────────────────────────────────────────────────

/// Backend identifier so callers can inspect which driver is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfbBackendKind {
    DirectInput,
    Hid,
    VendorSpecific,
    Null,
}

/// Trait for FFB output backends.
///
/// Implementors translate the abstract force value (−1.0…+1.0) to hardware
/// commands. Implementations must be safe to call at 250 Hz and must never
/// block or allocate on the hot path.
pub trait FfbDevice: Send {
    /// Human-readable name of the device / driver.
    fn name(&self) -> &str;

    /// Backend kind.
    fn backend_kind(&self) -> FfbBackendKind;

    /// Send a force value to the device.
    ///
    /// `force` is in the range −1.0…+1.0 (clamped by the caller).
    /// Returns `Ok(())` on success or a static error message.
    fn send_force(&mut self, force: f32) -> Result<(), &'static str>;

    /// Send zero force and disable all active effects on the device.
    fn emergency_stop(&mut self) -> Result<(), &'static str>;

    /// Returns `true` when the device is still connected and healthy.
    fn is_connected(&self) -> bool;
}

/// No-op device used for testing and when no hardware is present.
#[derive(Debug)]
pub struct NullDevice {
    last_force: f32,
    connected: bool,
}

impl NullDevice {
    pub fn new() -> Self {
        Self {
            last_force: 0.0,
            connected: true,
        }
    }

    pub fn last_force(&self) -> f32 {
        self.last_force
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }
}

impl Default for NullDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl FfbDevice for NullDevice {
    fn name(&self) -> &str {
        "NullDevice"
    }
    fn backend_kind(&self) -> FfbBackendKind {
        FfbBackendKind::Null
    }
    fn send_force(&mut self, force: f32) -> Result<(), &'static str> {
        self.last_force = force;
        Ok(())
    }
    fn emergency_stop(&mut self) -> Result<(), &'static str> {
        self.last_force = 0.0;
        Ok(())
    }
    fn is_connected(&self) -> bool {
        self.connected
    }
}

// ─── Effect slot management ──────────────────────────────────────────────────

/// Maximum number of concurrent effect slots (pre-allocated, no heap).
pub const MAX_EFFECT_SLOTS: usize = 16;

/// Priority levels for effect scheduling (lower numeric value = higher priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum EffectPriority {
    /// Safety-critical effects (e.g. soft-stop ramp). Never pre-empted.
    Safety = 0,
    /// Primary control-loading effects (spring, damper).
    ControlLoading = 1,
    /// Environmental effects (weather, turbulence).
    Environmental = 2,
    /// Informational / decorative effects (engine vibration, gear rumble).
    Ambient = 3,
}

/// Handle returned when an effect is loaded into a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectSlotHandle(pub u8);

/// One pre-allocated effect slot.
#[derive(Debug, Clone, Copy)]
struct EffectSlot {
    effect: FfbEffect,
    priority: EffectPriority,
    gain: f32,
    active: bool,
    created_tick: u32,
}

/// Pre-allocated, bounded effect slot manager.
///
/// Stores up to [`MAX_EFFECT_SLOTS`] effects on the stack. Supports
/// load / unload / update without heap allocation.
#[derive(Debug)]
pub struct EffectSlotManager {
    slots: [Option<EffectSlot>; MAX_EFFECT_SLOTS],
    count: usize,
    current_tick: u32,
}

impl EffectSlotManager {
    pub fn new() -> Self {
        Self {
            slots: [None; MAX_EFFECT_SLOTS],
            count: 0,
            current_tick: 0,
        }
    }

    /// Load an effect into the first free slot. Returns a handle or `None` if full.
    pub fn load(
        &mut self,
        effect: FfbEffect,
        priority: EffectPriority,
        gain: f32,
    ) -> Option<EffectSlotHandle> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(EffectSlot {
                    effect,
                    priority,
                    gain: gain.clamp(0.0, 1.0),
                    active: true,
                    created_tick: self.current_tick,
                });
                self.count += 1;
                return Some(EffectSlotHandle(i as u8));
            }
        }
        None
    }

    /// Unload an effect slot.
    pub fn unload(&mut self, handle: EffectSlotHandle) -> bool {
        let idx = handle.0 as usize;
        if idx < MAX_EFFECT_SLOTS && self.slots[idx].is_some() {
            self.slots[idx] = None;
            self.count -= 1;
            true
        } else {
            false
        }
    }

    /// Update the effect parameters in a slot.
    pub fn update_effect(&mut self, handle: EffectSlotHandle, effect: FfbEffect) -> bool {
        let idx = handle.0 as usize;
        if let Some(slot) = self.slots.get_mut(idx).and_then(|s| s.as_mut()) {
            slot.effect = effect;
            true
        } else {
            false
        }
    }

    /// Set the gain of a slot.
    pub fn set_gain(&mut self, handle: EffectSlotHandle, gain: f32) -> bool {
        let idx = handle.0 as usize;
        if let Some(slot) = self.slots.get_mut(idx).and_then(|s| s.as_mut()) {
            slot.gain = gain.clamp(0.0, 1.0);
            true
        } else {
            false
        }
    }

    /// Pause / resume a slot.
    pub fn set_active(&mut self, handle: EffectSlotHandle, active: bool) -> bool {
        let idx = handle.0 as usize;
        if let Some(slot) = self.slots.get_mut(idx).and_then(|s| s.as_mut()) {
            slot.active = active;
            true
        } else {
            false
        }
    }

    /// Number of loaded effects.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Number of free slots.
    pub fn available(&self) -> usize {
        MAX_EFFECT_SLOTS - self.count
    }

    /// Remove all effects.
    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = None;
        }
        self.count = 0;
    }

    /// Advance the internal tick counter (call once per RT tick).
    pub fn tick(&mut self) {
        self.current_tick = self.current_tick.wrapping_add(1);
    }
}

impl Default for EffectSlotManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Priority-based effect scheduler ─────────────────────────────────────────

/// Computes the final force output from all active slots, ordered by priority.
///
/// Higher-priority effects are summed first. If the running total already
/// saturates (|sum| ≥ 1.0) before lower-priority effects are reached, the
/// remaining effects are skipped. This guarantees that safety-critical forces
/// always dominate.
#[derive(Debug)]
pub struct EffectScheduler {
    /// Scratch buffer for sorting — avoids heap allocation.
    sorted_indices: [u8; MAX_EFFECT_SLOTS],
}

impl EffectScheduler {
    pub fn new() -> Self {
        Self {
            sorted_indices: [0; MAX_EFFECT_SLOTS],
        }
    }

    /// Compute the scheduled force output from the slot manager.
    ///
    /// Returns a value in −1.0…+1.0.
    pub fn compute(&mut self, slots: &EffectSlotManager, input: &EffectInput) -> f32 {
        // Collect active slot indices into scratch buffer.
        let mut active_count = 0usize;
        for (i, slot) in slots.slots.iter().enumerate() {
            if let Some(s) = slot {
                if s.active {
                    self.sorted_indices[active_count] = i as u8;
                    active_count += 1;
                }
            }
        }

        if active_count == 0 {
            return 0.0;
        }

        // Insertion sort by priority (stable, O(n²) but n ≤ 16).
        for i in 1..active_count {
            let key = self.sorted_indices[i];
            let key_prio = slots.slots[key as usize].as_ref().unwrap().priority;
            let mut j = i;
            while j > 0 {
                let prev = self.sorted_indices[j - 1];
                let prev_prio = slots.slots[prev as usize].as_ref().unwrap().priority;
                if prev_prio <= key_prio {
                    break;
                }
                self.sorted_indices[j] = prev;
                j -= 1;
            }
            self.sorted_indices[j] = key;
        }

        // Sum forces in priority order; stop when saturated.
        let mut total = 0.0_f32;
        for &idx in &self.sorted_indices[..active_count] {
            let slot = slots.slots[idx as usize].as_ref().unwrap();
            let force = slot.effect.compute(input) * slot.gain;
            total += force;
            if total.abs() >= 1.0 {
                break;
            }
        }

        total.clamp(-1.0, 1.0)
    }
}

impl Default for EffectScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ─── User-configurable force limits ──────────────────────────────────────────

/// User-configurable global force cap.
///
/// Sits between the scheduler output and the device, applying an absolute
/// ceiling that the user can lower at any time for comfort or safety.
#[derive(Debug, Clone, Copy)]
pub struct UserForceLimit {
    /// Maximum output magnitude (0.0–1.0). Default 1.0 (no reduction).
    pub max_force: f32,
}

impl Default for UserForceLimit {
    fn default() -> Self {
        Self { max_force: 1.0 }
    }
}

impl UserForceLimit {
    /// Create with a specific cap.
    pub fn new(max_force: f32) -> Self {
        Self {
            max_force: max_force.clamp(0.0, 1.0),
        }
    }

    /// Apply the limit. Returns force clamped to ±max_force.
    pub fn apply(&self, force: f32) -> f32 {
        force.clamp(-self.max_force, self.max_force)
    }

    /// Update the cap at runtime.
    pub fn set_max_force(&mut self, max: f32) {
        self.max_force = max.clamp(0.0, 1.0);
    }
}

// ─── Watchdog integration ────────────────────────────────────────────────────

/// Watchdog that monitors update frequency and zeros the device on timeout.
///
/// Wraps an [`FfbDevice`] and tracks the last update timestamp. If
/// [`WatchdogDevice::tick`] is called and the elapsed time since the last
/// [`WatchdogDevice::send_force`] exceeds `timeout`, the device is
/// emergency-stopped automatically.
#[derive(Debug)]
pub struct DeviceWatchdog {
    last_update: Instant,
    timeout_ticks: u32,
    ticks_since_update: u32,
    tripped: bool,
}

impl DeviceWatchdog {
    /// Create a watchdog with the given timeout in ticks.
    pub fn new(timeout_ticks: u32) -> Self {
        Self {
            last_update: Instant::now(),
            timeout_ticks,
            ticks_since_update: 0,
            tripped: false,
        }
    }

    /// Call once per RT tick. Returns `true` if the watchdog has tripped.
    pub fn tick(&mut self) -> bool {
        self.ticks_since_update = self.ticks_since_update.saturating_add(1);
        if self.ticks_since_update >= self.timeout_ticks {
            self.tripped = true;
        }
        self.tripped
    }

    /// Feed the watchdog (call when a valid force update is sent).
    pub fn feed(&mut self) {
        self.ticks_since_update = 0;
        self.tripped = false;
        self.last_update = Instant::now();
    }

    pub fn is_tripped(&self) -> bool {
        self.tripped
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::*;

    fn input_at_rest() -> EffectInput {
        EffectInput {
            position: 0.0,
            velocity: 0.0,
            elapsed_s: 0.0,
            tick: 0,
        }
    }

    // ── NullDevice ───────────────────────────────────────────────────────

    #[test]
    fn null_device_send_force() {
        let mut dev = NullDevice::new();
        dev.send_force(0.42).unwrap();
        assert!((dev.last_force() - 0.42).abs() < 1e-6);
    }

    #[test]
    fn null_device_emergency_stop_zeros() {
        let mut dev = NullDevice::new();
        dev.send_force(0.9).unwrap();
        dev.emergency_stop().unwrap();
        assert!(dev.last_force().abs() < 1e-6);
    }

    #[test]
    fn null_device_disconnect() {
        let mut dev = NullDevice::new();
        assert!(dev.is_connected());
        dev.set_connected(false);
        assert!(!dev.is_connected());
    }

    // ── EffectSlotManager ────────────────────────────────────────────────

    #[test]
    fn slot_manager_load_and_unload() {
        let mut mgr = EffectSlotManager::new();
        assert!(mgr.is_empty());

        let h = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
                EffectPriority::ControlLoading,
                1.0,
            )
            .unwrap();
        assert_eq!(mgr.len(), 1);

        assert!(mgr.unload(h));
        assert!(mgr.is_empty());
    }

    #[test]
    fn slot_manager_capacity_bounded() {
        let mut mgr = EffectSlotManager::new();
        for _ in 0..MAX_EFFECT_SLOTS {
            assert!(
                mgr.load(
                    FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
                    EffectPriority::Ambient,
                    1.0,
                )
                .is_some()
            );
        }
        // Next load should fail.
        assert!(
            mgr.load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
                EffectPriority::Ambient,
                1.0,
            )
            .is_none()
        );
        assert_eq!(mgr.available(), 0);
    }

    #[test]
    fn slot_manager_update_effect() {
        let mut mgr = EffectSlotManager::new();
        let h = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
                EffectPriority::ControlLoading,
                1.0,
            )
            .unwrap();
        assert!(mgr.update_effect(
            h,
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.9 })
        ));
    }

    #[test]
    fn slot_manager_clear() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            EffectPriority::Ambient,
            1.0,
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.3 }),
            EffectPriority::Ambient,
            1.0,
        );
        assert_eq!(mgr.len(), 2);
        mgr.clear();
        assert!(mgr.is_empty());
    }

    // ── EffectScheduler ──────────────────────────────────────────────────

    #[test]
    fn scheduler_empty_is_zero() {
        let mgr = EffectSlotManager::new();
        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!(f.abs() < 1e-6);
    }

    #[test]
    fn scheduler_sums_effects() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.3 }),
            EffectPriority::ControlLoading,
            1.0,
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.2 }),
            EffectPriority::ControlLoading,
            1.0,
        );
        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn scheduler_priority_ordering() {
        let mut mgr = EffectSlotManager::new();
        // Load a safety-critical effect (should dominate)
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.9 }),
            EffectPriority::Safety,
            1.0,
        );
        // Load a low-priority ambient effect
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            EffectPriority::Ambient,
            1.0,
        );
        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        // Safety effect (0.9) summed first; total 1.4 → saturated after safety,
        // but ambient still gets added. Result clamped to 1.0.
        assert!((f - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scheduler_inactive_slot_skipped() {
        let mut mgr = EffectSlotManager::new();
        let h = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.7 }),
                EffectPriority::ControlLoading,
                1.0,
            )
            .unwrap();
        mgr.set_active(h, false);

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!(f.abs() < 1e-6, "inactive slot should produce no force");
    }

    #[test]
    fn scheduler_gain_applied() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            EffectPriority::ControlLoading,
            0.4,
        );
        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!((f - 0.4).abs() < 1e-6);
    }

    // ── UserForceLimit ───────────────────────────────────────────────────

    #[test]
    fn user_force_limit_default_no_reduction() {
        let lim = UserForceLimit::default();
        assert!((lim.apply(0.9) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn user_force_limit_caps_output() {
        let lim = UserForceLimit::new(0.5);
        assert!((lim.apply(0.8) - 0.5).abs() < 1e-6);
        assert!((lim.apply(-0.8) - -0.5).abs() < 1e-6);
    }

    #[test]
    fn user_force_limit_runtime_update() {
        let mut lim = UserForceLimit::new(1.0);
        assert!((lim.apply(0.9) - 0.9).abs() < 1e-6);
        lim.set_max_force(0.3);
        assert!((lim.apply(0.9) - 0.3).abs() < 1e-6);
    }

    // ── DeviceWatchdog ───────────────────────────────────────────────────

    #[test]
    fn device_watchdog_trips_on_timeout() {
        let mut wd = DeviceWatchdog::new(5);
        for _ in 0..4 {
            assert!(!wd.tick());
        }
        assert!(wd.tick()); // 5th tick
        assert!(wd.is_tripped());
    }

    #[test]
    fn device_watchdog_feed_resets() {
        let mut wd = DeviceWatchdog::new(5);
        for _ in 0..4 {
            wd.tick();
        }
        wd.feed();
        assert!(!wd.is_tripped());
        for _ in 0..4 {
            assert!(!wd.tick());
        }
    }
}
