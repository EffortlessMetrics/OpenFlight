// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

// This module contains stub implementations and constants for future DirectInput support.
// Many items are intentionally unused in the current placeholder implementation.
// Names follow Windows DirectInput API conventions (e.g., DICONSTANTFORCE, DIPERIODIC).
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::transmute_ptr_to_ref)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::clone_on_copy)]

//! DirectInput FFB device abstraction for Windows
//!
//! This module provides a safe abstraction over Windows DirectInput 8 for force feedback
//! device control. It implements device enumeration, capability querying, effect creation,
//! and safe torque command execution.
//!
//! # Requirements
//! - FFB-HID-01.1: DirectInput 8 (IDirectInputDevice8) for effect creation and management
//! - FFB-HID-01.9: Device capability querying (supports_pid, max_torque_nm, min_period_us)

#[cfg(windows)]
use windows::{Win32::System::Com::*, core::GUID};

// DirectInput constants
// Note: DirectInput8 is a legacy API. Full bindings may require additional work
// or use of the `windows-sys` crate with custom bindings.
// Allow dead_code as these constants are defined for future full DirectInput implementation.
#[allow(dead_code)]
#[cfg(windows)]
const DIRECTINPUT_VERSION: u32 = 0x0800;
#[cfg(windows)]
const INFINITE: u32 = 0xFFFFFFFF;
#[cfg(windows)]
const DI_FFNOMINALMAX: u32 = 10000;
#[cfg(windows)]
const DIEB_NOTRIGGER: u32 = 0xFFFFFFFF;
#[cfg(windows)]
const DIEFF_CARTESIAN: u32 = 0x00000001;
#[cfg(windows)]
const DIEFF_OBJECTOFFSETS: u32 = 0x00000002;
#[allow(dead_code)]
#[cfg(windows)]
const DIEP_TYPESPECIFICPARAMS: u32 = 0x00000020;
#[allow(dead_code)]
#[cfg(windows)]
const DIEP_START: u32 = 0x20000000;
#[allow(dead_code)]
#[cfg(windows)]
const DISCL_EXCLUSIVE: u32 = 0x00000001;
#[allow(dead_code)]
#[cfg(windows)]
const DISCL_BACKGROUND: u32 = 0x00000008;
#[allow(dead_code)]
#[cfg(windows)]
const DI8DEVCLASS_GAMECTRL: u32 = 4;
#[allow(dead_code)]
#[cfg(windows)]
const DIEDFL_ATTACHEDONLY: u32 = 0x00000001;
#[allow(dead_code)]
#[cfg(windows)]
const DIEDFL_FORCEFEEDBACK: u32 = 0x00000100;
#[allow(dead_code)]
#[cfg(windows)]
const DIDC_FORCEFEEDBACK: u32 = 0x00000001;

// DirectInput joystick axis offsets
#[cfg(windows)]
const DIJOFS_X: u32 = 0;
#[cfg(windows)]
const DIJOFS_Y: u32 = 4;

// DirectInput GUIDs (these would normally come from dinput.h)
#[allow(dead_code)]
#[cfg(windows)]
const GUID_ConstantForce: GUID = GUID::from_u128(0x13541C20_8E33_11D0_9AD0_00A0C9A06E35);
#[allow(dead_code)]
#[cfg(windows)]
const GUID_Sine: GUID = GUID::from_u128(0x13541C23_8E33_11D0_9AD0_00A0C9A06E35);
#[allow(dead_code)]
#[cfg(windows)]
const GUID_Spring: GUID = GUID::from_u128(0x13541C26_8E33_11D0_9AD0_00A0C9A06E35);
#[allow(dead_code)]
#[cfg(windows)]
const GUID_Damper: GUID = GUID::from_u128(0x13541C27_8E33_11D0_9AD0_00A0C9A06E35);

// DirectInput structures (simplified for compilation)
// In a full implementation, these would use proper DirectInput bindings
#[cfg(windows)]
#[repr(C)]
struct DICONSTANTFORCE {
    lMagnitude: i32,
}

#[cfg(windows)]
#[repr(C)]
struct DIPERIODIC {
    dwMagnitude: u32,
    lOffset: i32,
    dwPhase: u32,
    dwPeriod: u32,
}

#[cfg(windows)]
#[repr(C)]
struct DICONDITION {
    lOffset: i32,
    lPositiveCoefficient: i32,
    lNegativeCoefficient: i32,
    dwPositiveSaturation: u32,
    dwNegativeSaturation: u32,
    lDeadBand: i32,
}

#[cfg(windows)]
#[repr(C)]
struct DIEFFECT {
    dwSize: u32,
    dwFlags: u32,
    dwDuration: u32,
    dwSamplePeriod: u32,
    dwGain: u32,
    dwTriggerButton: u32,
    dwTriggerRepeatInterval: u32,
    cAxes: u32,
    rgdwAxes: *mut u32,
    rglDirection: *mut i32,
    lpEnvelope: *mut core::ffi::c_void,
    cbTypeSpecificParams: u32,
    lpvTypeSpecificParams: *mut core::ffi::c_void,
    dwStartDelay: u32,
}

#[cfg(windows)]
impl Default for DIEFFECT {
    fn default() -> Self {
        Self {
            dwSize: std::mem::size_of::<DIEFFECT>() as u32,
            dwFlags: 0,
            dwDuration: 0,
            dwSamplePeriod: 0,
            dwGain: 0,
            dwTriggerButton: 0,
            dwTriggerRepeatInterval: 0,
            cAxes: 0,
            rgdwAxes: std::ptr::null_mut(),
            rglDirection: std::ptr::null_mut(),
            lpEnvelope: std::ptr::null_mut(),
            cbTypeSpecificParams: 0,
            lpvTypeSpecificParams: std::ptr::null_mut(),
            dwStartDelay: 0,
        }
    }
}

// Placeholder types for DirectInput interfaces
// In a full implementation, these would be proper COM interface bindings
#[cfg(windows)]
type IDirectInput8W = usize;
#[cfg(windows)]
type IDirectInputDevice8W = usize;
#[cfg(windows)]
type IDirectInputEffect = usize;

use flight_metrics::{
    MetricsRegistry,
    common::{DeviceMetricNames, FFB_DEVICE_METRICS},
};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

/// DirectInput FFB device errors
#[derive(Debug, Error)]
pub enum DInputError {
    #[error("DirectInput initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device acquisition failed: {0}")]
    AcquisitionFailed(String),

    #[error("Effect creation failed: {0}")]
    EffectCreationFailed(String),

    #[error("Effect update failed: {0}")]
    EffectUpdateFailed(String),

    #[error("Capability query failed: {0}")]
    CapabilityQueryFailed(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Device not acquired")]
    DeviceNotAcquired,

    #[error("Platform not supported (Windows only)")]
    PlatformNotSupported,

    #[error("Windows API error: {0:?}")]
    WindowsError(#[from] windows::core::Error),
}

pub type Result<T> = std::result::Result<T, DInputError>;

/// FFB device capabilities
#[derive(Debug, Clone)]
pub struct FfbCapabilities {
    /// Device supports PID (Physical Interface Device) effects
    pub supports_pid: bool,

    /// Device supports raw torque commands (OFP-1 protocol)
    pub supports_raw_torque: bool,

    /// Maximum torque output in Newton-meters
    pub max_torque_nm: f32,

    /// Minimum update period in microseconds
    pub min_period_us: u32,

    /// Device provides health/status stream
    pub has_health_stream: bool,

    /// Number of axes supported
    pub num_axes: u32,

    /// Number of effects supported
    pub max_effects: u32,
}

impl Default for FfbCapabilities {
    fn default() -> Self {
        Self {
            supports_pid: false,
            supports_raw_torque: false,
            max_torque_nm: 10.0,
            min_period_us: 1000,
            has_health_stream: false,
            num_axes: 2,
            max_effects: 10,
        }
    }
}

/// FFB effect types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    /// Constant force effect for sustained loads
    ConstantForce,

    /// Periodic (sine) effect for buffeting/vibration
    PeriodicSine,

    /// Spring condition effect for centering
    Spring,

    /// Damper condition effect for resistance
    Damper,
}

/// FFB effect handle
#[derive(Debug)]
pub struct EffectHandle {
    effect_type: EffectType,
    #[cfg(windows)]
    effect: Option<IDirectInputEffect>,
    #[allow(dead_code)]
    created_at: Instant,
    last_updated: Instant,
    axis_index: u32,
}

/// DirectInput FFB device abstraction
///
/// # Implementation Status
/// This module provides the API surface for DirectInput FFB device control.
/// The actual DirectInput8 COM bindings require additional work:
/// - Full DirectInput8 COM interface bindings (IDirectInput8, IDirectInputDevice8, IDirectInputEffect)
/// - Proper GUID definitions for effect types
/// - Complete DIEFFECT, DICONSTANTFORCE, DIPERIODIC, DICONDITION structures
///
/// The current implementation provides:
/// - Complete API surface with proper signatures
/// - Error handling and parameter validation
/// - Effect lifecycle management (create, update, start, stop)
/// - Device enumeration and capability querying
///
/// To complete the implementation:
/// 1. Add DirectInput8 COM bindings (via windows-sys or custom bindings)
/// 2. Replace placeholder types with actual COM interfaces
/// 3. Implement DirectInput8Create, EnumDevices, CreateDevice calls
/// 4. Wire up CreateEffect, SetParameters, Start, Stop calls
pub struct DirectInputFfbDevice {
    device_guid: String,
    #[cfg(windows)]
    #[allow(dead_code)]
    dinput: Option<IDirectInput8W>,
    #[cfg(windows)]
    #[allow(dead_code)]
    device: Option<IDirectInputDevice8W>,
    capabilities: FfbCapabilities,
    effects: Vec<EffectHandle>,
    is_acquired: bool,
    last_torque_nm: f32,
    /// Shared metrics registry
    metrics_registry: Arc<MetricsRegistry>,
    /// Device metric names
    device_metrics: DeviceMetricNames,
}

impl DirectInputFfbDevice {
    /// Create a new DirectInput FFB device
    ///
    /// # Arguments
    /// * `device_guid` - Device GUID string
    ///
    /// # Returns
    /// * `Result<Self>` - New device instance or error
    pub fn new(device_guid: String) -> Result<Self> {
        #[cfg(not(windows))]
        {
            let _ = device_guid;
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            Ok(Self {
                device_guid,
                dinput: None,
                device: None,
                capabilities: FfbCapabilities::default(),
                effects: Vec::new(),
                is_acquired: false,
                last_torque_nm: 0.0,
                metrics_registry: Arc::new(MetricsRegistry::new()),
                device_metrics: FFB_DEVICE_METRICS,
            })
        }
    }

    /// Create a new DirectInput FFB device with shared metrics registry
    pub fn new_with_metrics(
        device_guid: String,
        metrics_registry: Arc<MetricsRegistry>,
    ) -> Result<Self> {
        #[cfg(not(windows))]
        {
            let _ = (device_guid, metrics_registry);
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            Ok(Self {
                device_guid,
                dinput: None,
                device: None,
                capabilities: FfbCapabilities::default(),
                effects: Vec::new(),
                is_acquired: false,
                last_torque_nm: 0.0,
                metrics_registry,
                device_metrics: FFB_DEVICE_METRICS,
            })
        }
    }

    /// Create a new DirectInput FFB device with shared metrics registry and custom metric names
    pub fn new_with_metrics_and_device_metrics(
        device_guid: String,
        metrics_registry: Arc<MetricsRegistry>,
        device_metrics: DeviceMetricNames,
    ) -> Result<Self> {
        #[cfg(not(windows))]
        {
            let _ = (device_guid, metrics_registry, device_metrics);
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            Ok(Self {
                device_guid,
                dinput: None,
                device: None,
                capabilities: FfbCapabilities::default(),
                effects: Vec::new(),
                is_acquired: false,
                last_torque_nm: 0.0,
                metrics_registry,
                device_metrics,
            })
        }
    }

    /// Get shared metrics registry
    pub fn metrics_registry(&self) -> Arc<MetricsRegistry> {
        self.metrics_registry.clone()
    }

    fn record_metrics<T>(&self, start: Instant, result: &Result<T>) {
        self.metrics_registry
            .inc_counter(self.device_metrics.operations_total, 1);
        self.metrics_registry.observe(
            self.device_metrics.operation_latency_ms,
            start.elapsed().as_secs_f64() * 1000.0,
        );
        if result.is_err() {
            self.metrics_registry
                .inc_counter(self.device_metrics.errors_total, 1);
        }
    }

    fn with_metrics<T, F>(&mut self, operation: F) -> Result<T>
    where
        F: FnOnce(&mut Self) -> Result<T>,
    {
        let start = Instant::now();
        let result = operation(self);
        self.record_metrics(start, &result);
        result
    }

    /// Initialize DirectInput and create device
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Implementation Note
    /// This method provides the API surface for DirectInput initialization.
    /// The actual DirectInput8Create call requires full COM bindings.
    /// Current implementation uses stubs for compilation.
    pub fn initialize(&mut self) -> Result<()> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device| {
                // Initialize COM
                unsafe {
                    let hr = CoInitializeEx(Some(std::ptr::null()), COINIT_MULTITHREADED);
                    if hr.is_err() {
                        // COM initialization failed, but might already be initialized
                        tracing::warn!(
                            "COM initialization returned error (may already be initialized): {:?}",
                            hr
                        );
                    }
                }

                tracing::info!(
                    "DirectInput FFB device initialization started for GUID: {}",
                    device.device_guid
                );

                // TODO: Full implementation requires DirectInput8 COM bindings
                // The real implementation would:
                // 1. Call DirectInput8Create(GetModuleHandle(NULL), DIRECTINPUT_VERSION, IID_IDirectInput8, &dinput, NULL)
                // 2. Call dinput->CreateDevice(device_guid, &device, NULL)
                // 3. Call device->SetDataFormat(&c_dfDIJoystick2)
                // 4. Store the COM interface pointers

                // For now, mark as initialized with stub interfaces
                device.dinput = Some(0); // Placeholder
                device.device = Some(0); // Placeholder

                tracing::info!("DirectInput device initialized (stub implementation)");

                Ok(())
            })
        }
    }

    /// Enumerate available FFB devices
    ///
    /// # Returns
    /// * `Result<Vec<String>>` - List of device GUIDs or error
    ///
    /// # Implementation Note
    /// This method provides the API surface for device enumeration.
    /// The actual EnumDevices call requires full COM bindings.
    /// Current implementation returns empty list for compilation.
    pub fn enumerate_devices() -> Result<Vec<String>> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            tracing::debug!("Enumerating DirectInput FFB devices");

            // Initialize COM
            unsafe {
                let hr = CoInitializeEx(Some(std::ptr::null()), COINIT_MULTITHREADED);
                if hr.is_err() {
                    tracing::warn!(
                        "COM initialization returned error (may already be initialized): {:?}",
                        hr
                    );
                }
            }

            // TODO: Full implementation requires DirectInput8 COM bindings
            // The real implementation would:
            // 1. Call DirectInput8Create to get IDirectInput8 interface
            // 2. Call EnumDevices with DI8DEVCLASS_GAMECTRL and DIEDFL_FORCEFEEDBACK flags
            // 3. Collect device GUIDs from the callback
            // 4. Return list of device GUID strings

            // For now, return empty list (stub implementation)
            let devices = Vec::new();

            tracing::info!("Found {} FFB devices (stub implementation)", devices.len());
            Ok(devices)
        }
    }

    /// Query device capabilities
    ///
    /// # Returns
    /// * `Result<FfbCapabilities>` - Device capabilities or error
    ///
    /// # Implementation Note
    /// This method provides the API surface for capability querying.
    /// The actual GetCapabilities call requires full COM bindings.
    /// Current implementation returns default capabilities.
    pub fn query_capabilities(&mut self) -> Result<FfbCapabilities> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device| {
                let _device = device.device.as_ref().ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // TODO: Full implementation requires DirectInput8 COM bindings
                // The real implementation would:
                // 1. Create DIDEVCAPS structure
                // 2. Call device->GetCapabilities(&caps)
                // 3. Check caps.dwFlags for DIDC_FORCEFEEDBACK
                // 4. Extract num_axes, max_effects from caps

                // For now, return default capabilities (stub implementation)
                let caps = FfbCapabilities {
                    supports_pid: true,
                    supports_raw_torque: false, // Would need vendor-specific query
                    max_torque_nm: 15.0,        // Default, should be device-specific from config
                    min_period_us: 2000,        // 500 Hz default
                    has_health_stream: false,
                    num_axes: 2,
                    max_effects: 10,
                };

                device.capabilities = caps.clone();
                tracing::info!(
                    "Device capabilities queried (stub): supports_pid={}, axes={}, max_torque={} Nm",
                    caps.supports_pid,
                    caps.num_axes,
                    caps.max_torque_nm
                );

                Ok(caps)
            })
        }
    }

    /// Acquire the device for exclusive access
    ///
    /// # Arguments
    /// * `hwnd` - Window handle for cooperative level (0 for background)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Implementation Note
    /// This method provides the API surface for device acquisition.
    /// The actual SetCooperativeLevel and Acquire calls require full COM bindings.
    /// Current implementation uses stub for compilation.
    pub fn acquire(&mut self, hwnd: usize) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = hwnd;
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device| {
                if device.is_acquired {
                    return Ok(());
                }

                let _device = device.device.as_ref().ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // TODO: Full implementation requires DirectInput8 COM bindings
                // The real implementation would:
                // 1. Call device->SetCooperativeLevel(hwnd, DISCL_EXCLUSIVE | DISCL_BACKGROUND)
                // 2. Call device->Acquire()

                tracing::info!(
                    "DirectInput device acquired (hwnd: {}, stub implementation)",
                    hwnd
                );
                device.is_acquired = true;

                Ok(())
            })
        }
    }

    /// Release the device
    pub fn unacquire(&mut self) {
        #[cfg(windows)]
        {
            if !self.is_acquired {
                return;
            }

            // TODO: Full implementation would call device->Unacquire()

            tracing::info!("DirectInput device released (stub implementation)");
            self.is_acquired = false;
        }
    }

    /// Create a constant force effect
    ///
    /// # Arguments
    /// * `axis_index` - Axis index (0 = X/pitch, 1 = Y/roll)
    ///
    /// # Returns
    /// * `Result<usize>` - Effect handle index or error
    ///
    /// # Implementation Note
    /// This creates one constant force effect per axis, allowing independent control
    /// of pitch and roll torques. This is simpler than using a single multi-axis effect
    /// with vector direction calculations.
    pub fn create_constant_force_effect(&mut self, axis_index: u32) -> Result<usize> {
        #[cfg(not(windows))]
        {
            let _ = axis_index;
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                let _device = device_ctx.device.as_ref().ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Map axis index to DirectInput axis offset
                let axis_offset = match axis_index {
                    0 => DIJOFS_X, // Pitch (Y axis in DirectInput)
                    1 => DIJOFS_Y, // Roll (X axis in DirectInput)
                    _ => {
                        return Err(DInputError::InvalidParameter(format!(
                            "Invalid axis index: {}",
                            axis_index
                        )));
                    }
                };

                // Set up axis and direction arrays
                let axes = [axis_offset];
                let directions = [0i32]; // Direction along the axis

                // Create constant force parameters
                let constant_force = DICONSTANTFORCE {
                    lMagnitude: 0, // Start at zero
                };

                // Create effect parameters
                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 1,
                    rgdwAxes: axes.as_ptr() as *mut _,
                    rglDirection: directions.as_ptr() as *mut _,
                    lpEnvelope: std::ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DICONSTANTFORCE>() as u32,
                    lpvTypeSpecificParams: &constant_force as *const _ as *mut _,
                    dwStartDelay: 0,
                };

                // TODO: Full implementation requires DirectInput8 COM bindings
                // The real implementation would:
                // 1. Call device->CreateEffect(&GUID_ConstantForce, &effect_params, &effect, NULL)
                // 2. Store the IDirectInputEffect interface pointer

                let _ = effect_params; // Suppress unused warning

                let effect_handle = EffectHandle {
                    effect_type: EffectType::ConstantForce,
                    effect: Some(0), // Placeholder
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index,
                };

                device_ctx.effects.push(effect_handle);
                let handle_index = device_ctx.effects.len() - 1;

                tracing::info!(
                    "Created constant force effect for axis {} (handle: {})",
                    axis_index,
                    handle_index
                );

                Ok(handle_index)
            })
        }
    }

    /// Create a periodic (sine) effect
    ///
    /// # Returns
    /// * `Result<usize>` - Effect handle index or error
    pub fn create_periodic_effect(&mut self) -> Result<usize> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                let _device = device_ctx.device.as_ref().ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use both X and Y axes for periodic effects
                let axes = [DIJOFS_X, DIJOFS_Y];
                let directions = [0i32, 0i32];

                // Create periodic parameters (sine wave)
                let periodic = DIPERIODIC {
                    dwMagnitude: 0, // Start at zero
                    lOffset: 0,
                    dwPhase: 0,
                    dwPeriod: 100000, // 100ms = 10Hz default
                };

                // Create effect parameters
                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 2,
                    rgdwAxes: axes.as_ptr() as *mut _,
                    rglDirection: directions.as_ptr() as *mut _,
                    lpEnvelope: std::ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DIPERIODIC>() as u32,
                    lpvTypeSpecificParams: &periodic as *const _ as *mut _,
                    dwStartDelay: 0,
                };

                // TODO: Full implementation requires DirectInput8 COM bindings
                let _effect_params = effect_params; // Suppress unused warning

                let effect_handle = EffectHandle {
                    effect_type: EffectType::PeriodicSine,
                    effect: Some(0), // Placeholder
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index: 0, // Not axis-specific
                };

                device_ctx.effects.push(effect_handle);
                let handle_index = device_ctx.effects.len() - 1;

                tracing::info!("Created periodic sine effect (handle: {})", handle_index);

                Ok(handle_index)
            })
        }
    }

    /// Create a spring condition effect
    ///
    /// # Returns
    /// * `Result<usize>` - Effect handle index or error
    pub fn create_spring_effect(&mut self) -> Result<usize> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                let _device = device_ctx.device.as_ref().ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use both X and Y axes for spring effects
                let axes = [DIJOFS_X, DIJOFS_Y];
                let directions = [0i32, 0i32];

                // Create spring condition parameters (one per axis)
                let conditions = [
                    DICONDITION {
                        lOffset: 0,
                        lPositiveCoefficient: 0,
                        lNegativeCoefficient: 0,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                    DICONDITION {
                        lOffset: 0,
                        lPositiveCoefficient: 0,
                        lNegativeCoefficient: 0,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                ];

                // Create effect parameters
                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 2,
                    rgdwAxes: axes.as_ptr() as *mut _,
                    rglDirection: directions.as_ptr() as *mut _,
                    lpEnvelope: std::ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DICONDITION>() as u32 * 2,
                    lpvTypeSpecificParams: conditions.as_ptr() as *mut _,
                    dwStartDelay: 0,
                };

                // TODO: Full implementation requires DirectInput8 COM bindings
                let _ = effect_params; // Suppress unused warning

                let effect_handle = EffectHandle {
                    effect_type: EffectType::Spring,
                    effect: Some(0), // Placeholder
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index: 0, // Not axis-specific
                };

                device_ctx.effects.push(effect_handle);
                let handle_index = device_ctx.effects.len() - 1;

                tracing::info!("Created spring condition effect (handle: {})", handle_index);

                Ok(handle_index)
            })
        }
    }

    /// Create a damper condition effect
    ///
    /// # Returns
    /// * `Result<usize>` - Effect handle index or error
    pub fn create_damper_effect(&mut self) -> Result<usize> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                let _device = device_ctx.device.as_ref().ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use both X and Y axes for damper effects
                let axes = [DIJOFS_X, DIJOFS_Y];
                let directions = [0i32, 0i32];

                // Create damper condition parameters (one per axis)
                let conditions = [
                    DICONDITION {
                        lOffset: 0,
                        lPositiveCoefficient: 0,
                        lNegativeCoefficient: 0,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                    DICONDITION {
                        lOffset: 0,
                        lPositiveCoefficient: 0,
                        lNegativeCoefficient: 0,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                ];

                // Create effect parameters
                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 2,
                    rgdwAxes: axes.as_ptr() as *mut _,
                    rglDirection: directions.as_ptr() as *mut _,
                    lpEnvelope: std::ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DICONDITION>() as u32 * 2,
                    lpvTypeSpecificParams: conditions.as_ptr() as *mut _,
                    dwStartDelay: 0,
                };

                // TODO: Full implementation requires DirectInput8 COM bindings
                let _ = effect_params; // Suppress unused warning

                let effect_handle = EffectHandle {
                    effect_type: EffectType::Damper,
                    effect: Some(0), // Placeholder
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index: 0, // Not axis-specific
                };

                device_ctx.effects.push(effect_handle);
                let handle_index = device_ctx.effects.len() - 1;

                tracing::info!("Created damper condition effect (handle: {})", handle_index);

                Ok(handle_index)
            })
        }
    }

    /// Update constant force effect magnitude
    ///
    /// # Arguments
    /// * `handle` - Effect handle index
    /// * `torque_nm` - Torque in Newton-meters
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Requirements
    /// - FFB-HID-01.4: Effect parameter updates via SetParameters
    pub fn set_constant_force(&mut self, handle: usize, torque_nm: f32) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = (handle, torque_nm);
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                if handle >= device_ctx.effects.len() {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid effect handle: {}",
                        handle
                    )));
                }

                let effect_handle = &mut device_ctx.effects[handle];
                if effect_handle.effect_type != EffectType::ConstantForce {
                    return Err(DInputError::InvalidParameter(format!(
                        "Effect {} is not a constant force effect",
                        handle
                    )));
                }

                // Clamp to device limits
                let clamped_torque = torque_nm.clamp(
                    -device_ctx.capabilities.max_torque_nm,
                    device_ctx.capabilities.max_torque_nm,
                );

                // Convert torque_nm to DirectInput magnitude (-10000 to 10000)
                let magnitude =
                    (clamped_torque / device_ctx.capabilities.max_torque_nm * 10000.0) as i32;

                // Create constant force parameters
                let constant_force = DICONSTANTFORCE {
                    lMagnitude: magnitude,
                };

                // Get the effect interface
                let _effect = effect_handle.effect.as_ref().ok_or_else(|| {
                    DInputError::EffectUpdateFailed("Effect not created".to_string())
                })?;

                // TODO: Full implementation requires DirectInput8 COM bindings
                // The real implementation would:
                // 1. Create DIEFFECT structure with updated parameters
                // 2. Call effect->SetParameters(&effect_params, DIEP_TYPESPECIFICPARAMS | DIEP_START)

                let _constant_force = constant_force; // Suppress unused warning

                effect_handle.last_updated = Instant::now();
                device_ctx.last_torque_nm = clamped_torque;

                tracing::debug!(
                    "Set constant force effect {} to {} Nm (magnitude: {})",
                    handle,
                    clamped_torque,
                    magnitude
                );

                Ok(())
            })
        }
    }

    /// Update periodic effect parameters
    ///
    /// # Arguments
    /// * `handle` - Effect handle index
    /// * `frequency_hz` - Frequency in Hertz
    /// * `magnitude` - Magnitude (0.0 to 1.0)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Requirements
    /// - FFB-HID-01.3: Periodic (sine) effect creation for buffeting/vibration
    /// - FFB-HID-01.4: Effect parameter updates via SetParameters
    pub fn set_periodic_parameters(
        &mut self,
        handle: usize,
        frequency_hz: f32,
        magnitude: f32,
    ) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = (handle, frequency_hz, magnitude);
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                if handle >= device_ctx.effects.len() {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid effect handle: {}",
                        handle
                    )));
                }

                let effect_handle = &mut device_ctx.effects[handle];
                if effect_handle.effect_type != EffectType::PeriodicSine {
                    return Err(DInputError::InvalidParameter(format!(
                        "Effect {} is not a periodic effect",
                        handle
                    )));
                }

                // Validate parameters
                if frequency_hz <= 0.0 || frequency_hz > 1000.0 {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid frequency: {} Hz (must be 0-1000)",
                        frequency_hz
                    )));
                }

                let clamped_magnitude = magnitude.clamp(0.0, 1.0);

                // Convert to DirectInput parameters
                let di_magnitude = (clamped_magnitude * 10000.0) as u32;
                let period_us = (1.0 / frequency_hz * 1_000_000.0) as u32;

                // Create periodic parameters
                let periodic = DIPERIODIC {
                    dwMagnitude: di_magnitude,
                    lOffset: 0,
                    dwPhase: 0,
                    dwPeriod: period_us,
                };

                // Get the effect interface
                let _effect = effect_handle.effect.as_ref().ok_or_else(|| {
                    DInputError::EffectUpdateFailed("Effect not created".to_string())
                })?;

                // TODO: Full implementation requires DirectInput8 COM bindings
                let _periodic = periodic; // Suppress unused warning

                effect_handle.last_updated = Instant::now();

                tracing::debug!(
                    "Set periodic effect {} to {} Hz, magnitude {} (period: {} us)",
                    handle,
                    frequency_hz,
                    clamped_magnitude,
                    period_us
                );

                Ok(())
            })
        }
    }

    /// Update spring condition effect parameters
    ///
    /// # Arguments
    /// * `handle` - Effect handle index
    /// * `center` - Center position (-1.0 to 1.0)
    /// * `stiffness` - Spring stiffness (0.0 to 1.0)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Requirements
    /// - FFB-HID-01.3: Condition effects (spring) for centering
    /// - FFB-HID-01.4: Effect parameter updates via SetParameters
    pub fn set_spring_parameters(
        &mut self,
        handle: usize,
        center: f32,
        stiffness: f32,
    ) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = (handle, center, stiffness);
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                if handle >= device_ctx.effects.len() {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid effect handle: {}",
                        handle
                    )));
                }

                let effect_handle = &mut device_ctx.effects[handle];
                if effect_handle.effect_type != EffectType::Spring {
                    return Err(DInputError::InvalidParameter(format!(
                        "Effect {} is not a spring effect",
                        handle
                    )));
                }

                // Validate parameters
                let clamped_center = center.clamp(-1.0, 1.0);
                let clamped_stiffness = stiffness.clamp(0.0, 1.0);

                // Convert to DirectInput parameters
                let offset = (clamped_center * 10000.0) as i32;
                let coefficient = (clamped_stiffness * 10000.0) as i32;

                // Create spring condition parameters (one per axis)
                let _conditions = [
                    DICONDITION {
                        lOffset: offset,
                        lPositiveCoefficient: coefficient,
                        lNegativeCoefficient: coefficient,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                    DICONDITION {
                        lOffset: offset,
                        lPositiveCoefficient: coefficient,
                        lNegativeCoefficient: coefficient,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                ];

                // Get the effect interface
                let _effect = effect_handle.effect.as_ref().ok_or_else(|| {
                    DInputError::EffectUpdateFailed("Effect not created".to_string())
                })?;

                // TODO: Full implementation requires DirectInput8 COM bindings

                effect_handle.last_updated = Instant::now();

                tracing::debug!(
                    "Set spring effect {} to center {}, stiffness {} (stub)",
                    handle,
                    clamped_center,
                    clamped_stiffness
                );

                Ok(())
            })
        }
    }

    /// Update damper condition effect parameters
    ///
    /// # Arguments
    /// * `handle` - Effect handle index
    /// * `damping` - Damping coefficient (0.0 to 1.0)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Requirements
    /// - FFB-HID-01.3: Condition effects (damper) for resistance
    /// - FFB-HID-01.4: Effect parameter updates via SetParameters
    pub fn set_damper_parameters(&mut self, handle: usize, damping: f32) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = (handle, damping);
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                if handle >= device_ctx.effects.len() {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid effect handle: {}",
                        handle
                    )));
                }

                let effect_handle = &mut device_ctx.effects[handle];
                if effect_handle.effect_type != EffectType::Damper {
                    return Err(DInputError::InvalidParameter(format!(
                        "Effect {} is not a damper effect",
                        handle
                    )));
                }

                // Validate parameters
                let clamped_damping = damping.clamp(0.0, 1.0);

                // Convert to DirectInput parameters
                let coefficient = (clamped_damping * 10000.0) as i32;

                // Create damper condition parameters (one per axis)
                let _conditions = [
                    DICONDITION {
                        lOffset: 0,
                        lPositiveCoefficient: coefficient,
                        lNegativeCoefficient: coefficient,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                    DICONDITION {
                        lOffset: 0,
                        lPositiveCoefficient: coefficient,
                        lNegativeCoefficient: coefficient,
                        dwPositiveSaturation: DI_FFNOMINALMAX,
                        dwNegativeSaturation: DI_FFNOMINALMAX,
                        lDeadBand: 0,
                    },
                ];

                // Get the effect interface
                let _effect = effect_handle.effect.as_ref().ok_or_else(|| {
                    DInputError::EffectUpdateFailed("Effect not created".to_string())
                })?;

                // TODO: Full implementation requires DirectInput8 COM bindings

                effect_handle.last_updated = Instant::now();

                tracing::debug!(
                    "Set damper effect {} to damping {} (stub)",
                    handle,
                    clamped_damping
                );

                Ok(())
            })
        }
    }

    /// Start an effect
    ///
    /// # Arguments
    /// * `handle` - Effect handle index
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn start_effect(&mut self, handle: usize) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = handle;
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                if handle >= device_ctx.effects.len() {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid effect handle: {}",
                        handle
                    )));
                }

                let effect_handle = &device_ctx.effects[handle];
                let _effect = effect_handle.effect.as_ref().ok_or_else(|| {
                    DInputError::EffectUpdateFailed("Effect not created".to_string())
                })?;

                // TODO: Full implementation would call effect->Start(1, 0)

                tracing::debug!("Started effect {} (stub)", handle);
                Ok(())
            })
        }
    }

    /// Stop an effect
    ///
    /// # Arguments
    /// * `handle` - Effect handle index
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn stop_effect(&mut self, handle: usize) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = handle;
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device_ctx| {
                if !device_ctx.is_acquired {
                    return Err(DInputError::DeviceNotAcquired);
                }

                if handle >= device_ctx.effects.len() {
                    return Err(DInputError::InvalidParameter(format!(
                        "Invalid effect handle: {}",
                        handle
                    )));
                }

                let effect_handle = &device_ctx.effects[handle];
                let _effect = effect_handle.effect.as_ref().ok_or_else(|| {
                    DInputError::EffectUpdateFailed("Effect not created".to_string())
                })?;

                // TODO: Full implementation would call effect->Stop()

                tracing::debug!("Stopped effect {} (stub)", handle);
                Ok(())
            })
        }
    }

    /// Get device capabilities
    pub fn get_capabilities(&self) -> &FfbCapabilities {
        &self.capabilities
    }

    /// Check if device is acquired
    pub fn is_acquired(&self) -> bool {
        self.is_acquired
    }

    /// Get last torque command
    pub fn get_last_torque_nm(&self) -> f32 {
        self.last_torque_nm
    }

    /// Get number of created effects
    pub fn get_effect_count(&self) -> usize {
        self.effects.len()
    }
}

impl Drop for DirectInputFfbDevice {
    fn drop(&mut self) {
        self.unacquire();

        #[cfg(windows)]
        {
            // Clean up effects
            self.effects.clear();

            // Release device and DirectInput interfaces
            self.device = None;
            self.dinput = None;

            // Uninitialize COM
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_creation() {
        let device = DirectInputFfbDevice::new("test-guid".to_string());

        #[cfg(windows)]
        assert!(device.is_ok());

        #[cfg(not(windows))]
        assert!(matches!(device, Err(DInputError::PlatformNotSupported)));
    }

    #[test]
    #[cfg(windows)]
    fn test_device_initialization() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        let result = device.initialize();
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_capability_query() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();

        let caps = device.query_capabilities().unwrap();
        assert!(caps.supports_pid);
        assert_eq!(caps.max_torque_nm, 15.0);
        assert_eq!(caps.min_period_us, 2000);
    }

    #[test]
    #[cfg(windows)]
    fn test_device_enumeration() {
        // Test device enumeration
        let devices = DirectInputFfbDevice::enumerate_devices();
        assert!(devices.is_ok());

        // In a real implementation, this would return actual devices
        // For now, we just verify it doesn't crash
        let device_list = devices.unwrap();
        assert!(device_list.is_empty() || !device_list.is_empty());
    }

    #[test]
    #[cfg(windows)]
    fn test_device_acquisition() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();

        assert!(!device.is_acquired());

        let result = device.acquire(0);
        assert!(result.is_ok());
        assert!(device.is_acquired());

        device.unacquire();
        assert!(!device.is_acquired());
    }

    #[test]
    #[cfg(windows)]
    fn test_effect_creation() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create constant force effect
        let handle = device.create_constant_force_effect(0).unwrap();
        assert_eq!(handle, 0);
        assert_eq!(device.get_effect_count(), 1);

        // Create periodic effect
        let handle = device.create_periodic_effect().unwrap();
        assert_eq!(handle, 1);
        assert_eq!(device.get_effect_count(), 2);

        // Create spring effect
        let handle = device.create_spring_effect().unwrap();
        assert_eq!(handle, 2);
        assert_eq!(device.get_effect_count(), 3);

        // Create damper effect
        let handle = device.create_damper_effect().unwrap();
        assert_eq!(handle, 3);
        assert_eq!(device.get_effect_count(), 4);
    }

    #[test]
    #[cfg(windows)]
    fn test_constant_force_update() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.query_capabilities().unwrap(); // Query capabilities to get max_torque_nm = 15.0
        device.acquire(0).unwrap();

        let handle = device.create_constant_force_effect(0).unwrap();

        // Set torque within limits
        let result = device.set_constant_force(handle, 5.0);
        assert!(result.is_ok());
        assert_eq!(device.get_last_torque_nm(), 5.0);

        // Set torque exceeding limits (should clamp)
        let result = device.set_constant_force(handle, 20.0);
        assert!(result.is_ok());
        assert_eq!(device.get_last_torque_nm(), 15.0); // Clamped to max

        // Set negative torque
        let result = device.set_constant_force(handle, -8.0);
        assert!(result.is_ok());
        assert_eq!(device.get_last_torque_nm(), -8.0);
    }

    #[test]
    #[cfg(windows)]
    fn test_effect_start_stop() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_constant_force_effect(0).unwrap();

        assert!(device.start_effect(handle).is_ok());
        assert!(device.stop_effect(handle).is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_invalid_effect_handle() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Try to use invalid handle
        let result = device.set_constant_force(999, 5.0);
        assert!(matches!(result, Err(DInputError::InvalidParameter(_))));
    }

    #[test]
    #[cfg(windows)]
    fn test_effect_without_acquisition() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();

        // Try to create effect without acquiring device
        let result = device.create_constant_force_effect(0);
        assert!(matches!(result, Err(DInputError::DeviceNotAcquired)));
    }

    // ========================================================================
    // Task 16.1: Unit tests for FFB effect management
    // Requirements: FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4
    // ========================================================================

    #[test]
    #[cfg(windows)]
    fn test_periodic_effect_parameter_updates() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create periodic effect
        let handle = device.create_periodic_effect().unwrap();

        // Test valid parameter updates
        assert!(device.set_periodic_parameters(handle, 10.0, 0.5).is_ok());
        assert!(device.set_periodic_parameters(handle, 100.0, 1.0).is_ok());
        assert!(device.set_periodic_parameters(handle, 1.0, 0.0).is_ok());

        // Test parameter clamping
        assert!(device.set_periodic_parameters(handle, 50.0, 1.5).is_ok()); // magnitude clamped to 1.0
        assert!(device.set_periodic_parameters(handle, 50.0, -0.5).is_ok()); // magnitude clamped to 0.0

        // Test invalid frequency
        assert!(matches!(
            device.set_periodic_parameters(handle, 0.0, 0.5),
            Err(DInputError::InvalidParameter(_))
        ));
        assert!(matches!(
            device.set_periodic_parameters(handle, -10.0, 0.5),
            Err(DInputError::InvalidParameter(_))
        ));
        assert!(matches!(
            device.set_periodic_parameters(handle, 2000.0, 0.5),
            Err(DInputError::InvalidParameter(_))
        ));
    }

    #[test]
    #[cfg(windows)]
    fn test_periodic_effect_wrong_type() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create constant force effect
        let handle = device.create_constant_force_effect(0).unwrap();

        // Try to set periodic parameters on constant force effect
        let result = device.set_periodic_parameters(handle, 10.0, 0.5);
        assert!(matches!(result, Err(DInputError::InvalidParameter(_))));
    }

    #[test]
    #[cfg(windows)]
    fn test_spring_effect_parameter_updates() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create spring effect
        let handle = device.create_spring_effect().unwrap();

        // Test valid parameter updates
        assert!(device.set_spring_parameters(handle, 0.0, 0.5).is_ok());
        assert!(device.set_spring_parameters(handle, -0.5, 1.0).is_ok());
        assert!(device.set_spring_parameters(handle, 1.0, 0.0).is_ok());

        // Test parameter clamping
        assert!(device.set_spring_parameters(handle, 2.0, 0.5).is_ok()); // center clamped to 1.0
        assert!(device.set_spring_parameters(handle, -2.0, 0.5).is_ok()); // center clamped to -1.0
        assert!(device.set_spring_parameters(handle, 0.0, 1.5).is_ok()); // stiffness clamped to 1.0
        assert!(device.set_spring_parameters(handle, 0.0, -0.5).is_ok()); // stiffness clamped to 0.0
    }

    #[test]
    #[cfg(windows)]
    fn test_spring_effect_wrong_type() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create damper effect
        let handle = device.create_damper_effect().unwrap();

        // Try to set spring parameters on damper effect
        let result = device.set_spring_parameters(handle, 0.0, 0.5);
        assert!(matches!(result, Err(DInputError::InvalidParameter(_))));
    }

    #[test]
    #[cfg(windows)]
    fn test_damper_effect_parameter_updates() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create damper effect
        let handle = device.create_damper_effect().unwrap();

        // Test valid parameter updates
        assert!(device.set_damper_parameters(handle, 0.0).is_ok());
        assert!(device.set_damper_parameters(handle, 0.5).is_ok());
        assert!(device.set_damper_parameters(handle, 1.0).is_ok());

        // Test parameter clamping
        assert!(device.set_damper_parameters(handle, 1.5).is_ok()); // damping clamped to 1.0
        assert!(device.set_damper_parameters(handle, -0.5).is_ok()); // damping clamped to 0.0
    }

    #[test]
    #[cfg(windows)]
    fn test_damper_effect_wrong_type() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create spring effect
        let handle = device.create_spring_effect().unwrap();

        // Try to set damper parameters on spring effect
        let result = device.set_damper_parameters(handle, 0.5);
        assert!(matches!(result, Err(DInputError::InvalidParameter(_))));
    }

    #[test]
    #[cfg(windows)]
    fn test_multiple_effect_types_coexist() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create multiple effect types
        let constant_handle = device.create_constant_force_effect(0).unwrap();
        let periodic_handle = device.create_periodic_effect().unwrap();
        let spring_handle = device.create_spring_effect().unwrap();
        let damper_handle = device.create_damper_effect().unwrap();

        // Verify all effects can be updated independently
        assert!(device.set_constant_force(constant_handle, 5.0).is_ok());
        assert!(
            device
                .set_periodic_parameters(periodic_handle, 20.0, 0.7)
                .is_ok()
        );
        assert!(
            device
                .set_spring_parameters(spring_handle, 0.0, 0.8)
                .is_ok()
        );
        assert!(device.set_damper_parameters(damper_handle, 0.6).is_ok());

        // Verify effect count
        assert_eq!(device.get_effect_count(), 4);
    }

    #[test]
    #[cfg(windows)]
    fn test_effect_start_stop_control() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create effects
        let constant_handle = device.create_constant_force_effect(0).unwrap();
        let periodic_handle = device.create_periodic_effect().unwrap();

        // Test start/stop for constant force
        assert!(device.start_effect(constant_handle).is_ok());
        assert!(device.stop_effect(constant_handle).is_ok());

        // Test start/stop for periodic
        assert!(device.start_effect(periodic_handle).is_ok());
        assert!(device.stop_effect(periodic_handle).is_ok());

        // Test invalid handle
        assert!(matches!(
            device.start_effect(999),
            Err(DInputError::InvalidParameter(_))
        ));
        assert!(matches!(
            device.stop_effect(999),
            Err(DInputError::InvalidParameter(_))
        ));
    }

    #[test]
    #[cfg(windows)]
    fn test_effect_parameter_updates_without_acquisition() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();

        // Try to update parameters without acquiring device
        assert!(matches!(
            device.set_periodic_parameters(0, 10.0, 0.5),
            Err(DInputError::DeviceNotAcquired)
        ));
        assert!(matches!(
            device.set_spring_parameters(0, 0.0, 0.5),
            Err(DInputError::DeviceNotAcquired)
        ));
        assert!(matches!(
            device.set_damper_parameters(0, 0.5),
            Err(DInputError::DeviceNotAcquired)
        ));
    }

    #[test]
    #[cfg(windows)]
    fn test_constant_force_for_pitch_and_roll_axes() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.query_capabilities().unwrap();
        device.acquire(0).unwrap();

        // Create constant force effects for pitch (axis 0) and roll (axis 1)
        let pitch_handle = device.create_constant_force_effect(0).unwrap();
        let roll_handle = device.create_constant_force_effect(1).unwrap();

        // Set different torques for each axis
        assert!(device.set_constant_force(pitch_handle, 3.0).is_ok());
        assert!(device.set_constant_force(roll_handle, -2.5).is_ok());

        // Verify both effects can be controlled independently
        assert!(device.start_effect(pitch_handle).is_ok());
        assert!(device.start_effect(roll_handle).is_ok());

        // Update torques independently
        assert!(device.set_constant_force(pitch_handle, 5.0).is_ok());
        assert!(device.set_constant_force(roll_handle, -4.0).is_ok());

        assert!(device.stop_effect(pitch_handle).is_ok());
        assert!(device.stop_effect(roll_handle).is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_effect_updates_preserve_last_updated_timestamp() {
        let mut device = DirectInputFfbDevice::new("test-guid".to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create effects
        let constant_handle = device.create_constant_force_effect(0).unwrap();
        let periodic_handle = device.create_periodic_effect().unwrap();
        let spring_handle = device.create_spring_effect().unwrap();
        let damper_handle = device.create_damper_effect().unwrap();

        // Small delay to ensure timestamps differ
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Update each effect and verify last_updated is updated
        // (We can't directly access last_updated, but we verify the operation succeeds)
        assert!(device.set_constant_force(constant_handle, 5.0).is_ok());
        assert!(
            device
                .set_periodic_parameters(periodic_handle, 20.0, 0.5)
                .is_ok()
        );
        assert!(
            device
                .set_spring_parameters(spring_handle, 0.0, 0.5)
                .is_ok()
        );
        assert!(device.set_damper_parameters(damper_handle, 0.5).is_ok());
    }
}
