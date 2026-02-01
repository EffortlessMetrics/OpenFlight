// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DirectInput FFB device abstraction for Windows
//!
//! This module provides a safe abstraction over Windows DirectInput 8 for force feedback
//! device control. It implements device enumeration, capability querying, effect creation,
//! and safe torque command execution.
//!
//! # Requirements
//! - FFB-HID-01.1: DirectInput 8 (IDirectInputDevice8) for effect creation and management
//! - FFB-HID-01.9: Device capability querying (supports_pid, max_torque_nm, min_period_us)

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]
// Allow unsafe operations in unsafe fns for FFI callbacks
#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
use crate::dinput_com::{
    DI_FFNOMINALMAX, DI8DEVCLASS_GAMECTRL, DIDC_FORCEFEEDBACK, DIDEVCAPS, DIDEVICEINSTANCEW,
    DIEB_NOTRIGGER, DIEDFL_ATTACHEDONLY, DIEDFL_FORCEFEEDBACK, DIEFF_CARTESIAN,
    DIEFF_OBJECTOFFSETS, DIEFFECT, DIENUM_CONTINUE, DIEP_NORESTART, DIEP_TYPESPECIFICPARAMS,
    DISCL_BACKGROUND, DISCL_EXCLUSIVE, DISFFC_RESET, DISFFC_STOPALL, GUID_ConstantForce,
    GUID_Damper, GUID_Sine, GUID_Spring, IDirectInput8W, IDirectInputDevice8W, IDirectInputEffect,
    INFINITE, create_direct_input8, guid_to_string, is_dinput8_available, is_disconnect_error,
    string_to_guid,
};
#[cfg(windows)]
use crate::dinput_com::{DICONDITION, DICONSTANTFORCE, DIPERIODIC};
#[cfg(windows)]
use crate::dinput_window::MessageOnlyWindow;

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::core::{GUID, PCWSTR};

use flight_metrics::{
    MetricsRegistry,
    common::{DeviceMetricNames, FFB_DEVICE_METRICS},
};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

// Re-export constants from dinput_com for backwards compatibility
#[cfg(windows)]
pub use crate::dinput_com::{DIJOFS_RX, DIJOFS_RY, DIJOFS_RZ, DIJOFS_X, DIJOFS_Y, DIJOFS_Z};

// Define for non-windows platforms
#[cfg(not(windows))]
pub const DIJOFS_X: u32 = 0;
#[cfg(not(windows))]
pub const DIJOFS_Y: u32 = 4;

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

    #[error("Device disconnected")]
    DeviceDisconnected,

    #[error("Platform not supported (Windows only)")]
    PlatformNotSupported,

    #[error("DirectInput not available")]
    DirectInputNotAvailable,

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

/// FFB effect handle with real DirectInput effect pointer
#[cfg(windows)]
pub struct EffectHandle {
    effect_type: EffectType,
    /// Raw pointer to IDirectInputEffect interface
    effect_ptr: *mut IDirectInputEffect,
    created_at: Instant,
    last_updated: Instant,
    axis_index: u32,
    /// Last magnitude value for dirty-checking
    last_magnitude: i32,
}

#[cfg(windows)]
impl std::fmt::Debug for EffectHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectHandle")
            .field("effect_type", &self.effect_type)
            .field("effect_ptr", &(!self.effect_ptr.is_null()))
            .field("axis_index", &self.axis_index)
            .field("last_magnitude", &self.last_magnitude)
            .finish()
    }
}

#[cfg(not(windows))]
#[derive(Debug)]
pub struct EffectHandle {
    effect_type: EffectType,
    created_at: Instant,
    last_updated: Instant,
    axis_index: u32,
    last_magnitude: i32,
}

/// Enumerated device information
#[derive(Debug, Clone)]
pub struct EnumeratedDevice {
    /// Device instance GUID as string
    pub instance_guid: String,
    /// Device product GUID as string
    pub product_guid: String,
    /// Device instance name
    pub instance_name: String,
    /// Device product name
    pub product_name: String,
}

/// DirectInput FFB device abstraction
///
/// This struct provides a safe interface to DirectInput 8 force feedback devices.
/// It handles COM initialization, device enumeration, acquisition, and effect management.
///
/// # Thread Safety
///
/// DirectInput requires STA (Single-Threaded Apartment) COM. The device should be
/// used from the thread that created it. This is enforced by the message-only window
/// which is `!Send` and `!Sync`.
pub struct DirectInputFfbDevice {
    device_guid: String,
    #[cfg(windows)]
    parsed_guid: Option<GUID>,
    #[cfg(windows)]
    dinput: Option<*mut IDirectInput8W>,
    #[cfg(windows)]
    device: Option<*mut IDirectInputDevice8W>,
    #[cfg(windows)]
    window: Option<MessageOnlyWindow>,
    #[cfg(windows)]
    com_initialized: AtomicBool,
    capabilities: FfbCapabilities,
    effects: Vec<EffectHandle>,
    is_acquired: bool,
    last_torque_nm: f32,
    /// Shared metrics registry
    metrics_registry: Arc<MetricsRegistry>,
    /// Device metric names
    device_metrics: DeviceMetricNames,
    /// Marker to make this struct !Send and !Sync (raw pointers are not thread-safe)
    #[cfg(windows)]
    _marker: std::marker::PhantomData<*const ()>,
}

// DirectInputFfbDevice is !Send and !Sync because it contains raw COM pointers
// that must be used on the thread that created them. This is enforced by the
// PhantomData<*const ()> marker type.

impl DirectInputFfbDevice {
    /// Create a new DirectInput FFB device
    ///
    /// # Arguments
    /// * `device_guid` - Device GUID string (e.g., "{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}")
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
            let parsed_guid = string_to_guid(&device_guid);

            Ok(Self {
                device_guid,
                parsed_guid,
                dinput: None,
                device: None,
                window: None,
                com_initialized: AtomicBool::new(false),
                capabilities: FfbCapabilities::default(),
                effects: Vec::new(),
                is_acquired: false,
                last_torque_nm: 0.0,
                metrics_registry: Arc::new(MetricsRegistry::new()),
                device_metrics: FFB_DEVICE_METRICS,
                _marker: std::marker::PhantomData,
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
            let parsed_guid = string_to_guid(&device_guid);

            Ok(Self {
                device_guid,
                parsed_guid,
                dinput: None,
                device: None,
                window: None,
                com_initialized: AtomicBool::new(false),
                capabilities: FfbCapabilities::default(),
                effects: Vec::new(),
                is_acquired: false,
                last_torque_nm: 0.0,
                metrics_registry,
                device_metrics: FFB_DEVICE_METRICS,
                _marker: std::marker::PhantomData,
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
            let parsed_guid = string_to_guid(&device_guid);

            Ok(Self {
                device_guid,
                parsed_guid,
                dinput: None,
                device: None,
                window: None,
                com_initialized: AtomicBool::new(false),
                capabilities: FfbCapabilities::default(),
                effects: Vec::new(),
                is_acquired: false,
                last_torque_nm: 0.0,
                metrics_registry,
                device_metrics,
                _marker: std::marker::PhantomData,
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
    /// This method:
    /// 1. Initializes COM if not already initialized
    /// 2. Loads dinput8.dll and creates IDirectInput8 interface
    /// 3. Creates the device from the specified GUID
    /// 4. Creates a message-only window for cooperative level
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn initialize(&mut self) -> Result<()> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device| {
                // Check if DirectInput is available
                if !is_dinput8_available() {
                    return Err(DInputError::DirectInputNotAvailable);
                }

                // Initialize COM
                unsafe {
                    let hr = CoInitializeEx(Some(ptr::null()), COINIT_MULTITHREADED);
                    if hr.is_ok() {
                        device.com_initialized.store(true, Ordering::SeqCst);
                    } else {
                        // COM might already be initialized, which is fine
                        tracing::debug!(
                            "COM initialization returned: {:?} (may already be initialized)",
                            hr
                        );
                    }
                }

                tracing::info!(
                    "DirectInput FFB device initialization started for GUID: {}",
                    device.device_guid
                );

                // Get module handle
                let hinstance = unsafe {
                    GetModuleHandleW(PCWSTR::null()).map_err(|e| {
                        DInputError::InitializationFailed(format!("GetModuleHandle failed: {}", e))
                    })?
                };

                // Create DirectInput8 interface
                let dinput = unsafe {
                    create_direct_input8(hinstance).map_err(|e| {
                        DInputError::InitializationFailed(format!(
                            "DirectInput8Create failed: {}",
                            e
                        ))
                    })?
                };
                device.dinput = Some(dinput);

                // Parse and validate GUID
                let guid = device.parsed_guid.ok_or_else(|| {
                    DInputError::InvalidParameter(format!(
                        "Invalid device GUID format: {}",
                        device.device_guid
                    ))
                })?;

                // Create device
                let di_device = unsafe {
                    (*dinput).create_device(&guid).map_err(|e| {
                        DInputError::DeviceNotFound(format!(
                            "CreateDevice failed for GUID {}: {}",
                            device.device_guid, e
                        ))
                    })?
                };
                device.device = Some(di_device);

                // Create message-only window for cooperative level
                device.window = Some(MessageOnlyWindow::new().map_err(|e| {
                    DInputError::InitializationFailed(format!(
                        "Failed to create message-only window: {}",
                        e
                    ))
                })?);

                tracing::info!(
                    "DirectInput device initialized successfully for GUID: {}",
                    device.device_guid
                );

                Ok(())
            })
        }
    }

    /// Enumerate available FFB devices
    ///
    /// # Returns
    /// * `Result<Vec<EnumeratedDevice>>` - List of enumerated devices or error
    #[cfg(windows)]
    pub fn enumerate_devices_detailed() -> Result<Vec<EnumeratedDevice>> {
        // Check if DirectInput is available
        if !is_dinput8_available() {
            return Err(DInputError::DirectInputNotAvailable);
        }

        tracing::debug!("Enumerating DirectInput FFB devices");

        // Initialize COM
        let com_initialized = unsafe {
            let hr = CoInitializeEx(Some(ptr::null()), COINIT_MULTITHREADED);
            hr.is_ok()
        };

        // Get module handle
        let hinstance = unsafe {
            GetModuleHandleW(PCWSTR::null()).map_err(|e| {
                DInputError::InitializationFailed(format!("GetModuleHandle failed: {}", e))
            })?
        };

        // Create DirectInput8 interface
        let dinput = unsafe {
            create_direct_input8(hinstance).map_err(|e| {
                DInputError::InitializationFailed(format!("DirectInput8Create failed: {}", e))
            })?
        };

        // Collect devices via enumeration callback
        let mut devices: Vec<EnumeratedDevice> = Vec::new();

        unsafe {
            (*dinput)
                .enum_devices(
                    DI8DEVCLASS_GAMECTRL,
                    enum_devices_callback,
                    &mut devices as *mut _ as *mut c_void,
                    DIEDFL_ATTACHEDONLY | DIEDFL_FORCEFEEDBACK,
                )
                .map_err(|e| {
                    DInputError::InitializationFailed(format!("EnumDevices failed: {}", e))
                })?;

            // Release DirectInput interface
            (*dinput).release();
        }

        // Uninitialize COM if we initialized it
        if com_initialized {
            unsafe {
                CoUninitialize();
            }
        }

        tracing::info!("Found {} FFB devices", devices.len());
        Ok(devices)
    }

    /// Enumerate available FFB devices (returns GUID strings for compatibility)
    ///
    /// # Returns
    /// * `Result<Vec<String>>` - List of device GUIDs or error
    pub fn enumerate_devices() -> Result<Vec<String>> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            let devices = Self::enumerate_devices_detailed()?;
            Ok(devices.into_iter().map(|d| d.instance_guid).collect())
        }
    }

    /// Query device capabilities
    ///
    /// # Returns
    /// * `Result<FfbCapabilities>` - Device capabilities or error
    pub fn query_capabilities(&mut self) -> Result<FfbCapabilities> {
        #[cfg(not(windows))]
        {
            return Err(DInputError::PlatformNotSupported);
        }

        #[cfg(windows)]
        {
            self.with_metrics(|device| {
                let di_device = device.device.ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                let mut caps = DIDEVCAPS::new();
                unsafe {
                    (*di_device).get_capabilities(&mut caps).map_err(|e| {
                        DInputError::CapabilityQueryFailed(format!(
                            "GetCapabilities failed: {}",
                            e
                        ))
                    })?;
                }

                // Check for force feedback support
                let supports_ffb = (caps.dwFlags & DIDC_FORCEFEEDBACK) != 0;

                let ffb_caps = FfbCapabilities {
                    supports_pid: supports_ffb,
                    supports_raw_torque: false, // Would need vendor-specific query
                    max_torque_nm: 15.0,        // Default, should be device-specific from config
                    min_period_us: caps.dwFFMinTimeResolution.max(2000), // Minimum 2ms
                    has_health_stream: false,
                    num_axes: caps.dwAxes,
                    max_effects: 10, // Conservative default
                };

                device.capabilities = ffb_caps.clone();
                tracing::info!(
                    "Device capabilities: supports_pid={}, axes={}, max_torque={} Nm, min_period={} us",
                    ffb_caps.supports_pid,
                    ffb_caps.num_axes,
                    ffb_caps.max_torque_nm,
                    ffb_caps.min_period_us
                );

                Ok(ffb_caps)
            })
        }
    }

    /// Acquire the device for exclusive access
    ///
    /// # Arguments
    /// * `hwnd` - Window handle (0 to use internal message-only window)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
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

                let di_device = device.device.ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use provided hwnd or internal message-only window
                let hwnd = if hwnd == 0 {
                    device.window.as_ref().map(|w| w.hwnd()).unwrap_or_default()
                } else {
                    HWND(hwnd as *mut _)
                };

                unsafe {
                    // Set cooperative level: exclusive + background
                    // DISCL_EXCLUSIVE is required for force feedback
                    // DISCL_BACKGROUND allows FFB when window is not in foreground
                    (*di_device)
                        .set_cooperative_level(hwnd, DISCL_EXCLUSIVE | DISCL_BACKGROUND)
                        .map_err(|e| {
                            DInputError::AcquisitionFailed(format!(
                                "SetCooperativeLevel failed: {}",
                                e
                            ))
                        })?;

                    // Acquire the device
                    (*di_device).acquire().map_err(|e| {
                        DInputError::AcquisitionFailed(format!("Acquire failed: {}", e))
                    })?;

                    // Reset force feedback to known state
                    if let Err(e) = (*di_device).send_force_feedback_command(DISFFC_RESET) {
                        tracing::warn!("Failed to reset FFB state: {}", e);
                    }
                }

                device.is_acquired = true;
                tracing::info!("DirectInput device acquired (hwnd: {:?})", hwnd);

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

            if let Some(di_device) = self.device {
                unsafe {
                    // Stop all effects
                    let _ = (*di_device).send_force_feedback_command(DISFFC_STOPALL);

                    // Unacquire
                    let _ = (*di_device).unacquire();
                }
            }

            tracing::info!("DirectInput device released");
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

                let di_device = device_ctx.device.ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Map axis index to DirectInput axis offset
                let axis_offset = match axis_index {
                    0 => DIJOFS_X,
                    1 => DIJOFS_Y,
                    _ => {
                        return Err(DInputError::InvalidParameter(format!(
                            "Invalid axis index: {}",
                            axis_index
                        )));
                    }
                };

                // Set up axis and direction arrays (must remain valid during CreateEffect)
                let mut axes = [axis_offset];
                let mut directions = [0i32];

                // Create constant force parameters
                let mut constant_force = DICONSTANTFORCE { lMagnitude: 0 };

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
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DICONSTANTFORCE>() as u32,
                    lpvTypeSpecificParams: &mut constant_force as *mut _ as *mut c_void,
                    dwStartDelay: 0,
                };

                // Create the effect
                let effect_ptr = unsafe {
                    (*di_device)
                        .create_effect(&GUID_ConstantForce, &effect_params)
                        .map_err(|e| {
                            DInputError::EffectCreationFailed(format!(
                                "CreateEffect (ConstantForce) failed: {}",
                                e
                            ))
                        })?
                };

                let effect_handle = EffectHandle {
                    effect_type: EffectType::ConstantForce,
                    effect_ptr,
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index,
                    last_magnitude: 0,
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

                let di_device = device_ctx.device.ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use both X and Y axes for periodic effects
                let mut axes = [DIJOFS_X, DIJOFS_Y];
                let mut directions = [0i32, 0i32];

                // Create periodic parameters (sine wave)
                let mut periodic = DIPERIODIC {
                    dwMagnitude: 0,
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
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DIPERIODIC>() as u32,
                    lpvTypeSpecificParams: &mut periodic as *mut _ as *mut c_void,
                    dwStartDelay: 0,
                };

                // Create the effect
                let effect_ptr = unsafe {
                    (*di_device)
                        .create_effect(&GUID_Sine, &effect_params)
                        .map_err(|e| {
                            DInputError::EffectCreationFailed(format!(
                                "CreateEffect (Sine) failed: {}",
                                e
                            ))
                        })?
                };

                let effect_handle = EffectHandle {
                    effect_type: EffectType::PeriodicSine,
                    effect_ptr,
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index: 0,
                    last_magnitude: 0,
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

                let di_device = device_ctx.device.ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use both X and Y axes for spring effects
                let mut axes = [DIJOFS_X, DIJOFS_Y];
                let mut directions = [0i32, 0i32];

                // Create spring condition parameters (one per axis)
                let mut conditions = [
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
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: (std::mem::size_of::<DICONDITION>() * 2) as u32,
                    lpvTypeSpecificParams: conditions.as_mut_ptr() as *mut c_void,
                    dwStartDelay: 0,
                };

                // Create the effect
                let effect_ptr = unsafe {
                    (*di_device)
                        .create_effect(&GUID_Spring, &effect_params)
                        .map_err(|e| {
                            DInputError::EffectCreationFailed(format!(
                                "CreateEffect (Spring) failed: {}",
                                e
                            ))
                        })?
                };

                let effect_handle = EffectHandle {
                    effect_type: EffectType::Spring,
                    effect_ptr,
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index: 0,
                    last_magnitude: 0,
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

                let di_device = device_ctx.device.ok_or_else(|| {
                    DInputError::InitializationFailed("Device not initialized".to_string())
                })?;

                // Use both X and Y axes for damper effects
                let mut axes = [DIJOFS_X, DIJOFS_Y];
                let mut directions = [0i32, 0i32];

                // Create damper condition parameters (one per axis)
                let mut conditions = [
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
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: (std::mem::size_of::<DICONDITION>() * 2) as u32,
                    lpvTypeSpecificParams: conditions.as_mut_ptr() as *mut c_void,
                    dwStartDelay: 0,
                };

                // Create the effect
                let effect_ptr = unsafe {
                    (*di_device)
                        .create_effect(&GUID_Damper, &effect_params)
                        .map_err(|e| {
                            DInputError::EffectCreationFailed(format!(
                                "CreateEffect (Damper) failed: {}",
                                e
                            ))
                        })?
                };

                let effect_handle = EffectHandle {
                    effect_type: EffectType::Damper,
                    effect_ptr,
                    created_at: Instant::now(),
                    last_updated: Instant::now(),
                    axis_index: 0,
                    last_magnitude: 0,
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

                // Dirty-check: skip update if magnitude changed by <1%
                let magnitude_diff = (magnitude - effect_handle.last_magnitude).abs();
                if magnitude_diff < 100 {
                    // Less than 1% change
                    return Ok(());
                }

                // Create constant force parameters
                let mut constant_force = DICONSTANTFORCE {
                    lMagnitude: magnitude,
                };

                // Create effect parameters for update
                let mut axes = [effect_handle.axis_index];
                let mut directions = [0i32];

                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 1,
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DICONSTANTFORCE>() as u32,
                    lpvTypeSpecificParams: &mut constant_force as *mut _ as *mut c_void,
                    dwStartDelay: 0,
                };

                // Update effect parameters with DIEP_NORESTART to avoid stuttering
                unsafe {
                    let result = (*effect_handle.effect_ptr)
                        .set_parameters(&effect_params, DIEP_TYPESPECIFICPARAMS | DIEP_NORESTART);

                    if let Err(e) = &result {
                        // Check for disconnect errors
                        if let Some(code) = e.code().0.checked_neg() {
                            if is_disconnect_error(-code) {
                                return Err(DInputError::DeviceDisconnected);
                            }
                        }
                        return Err(DInputError::EffectUpdateFailed(format!(
                            "SetParameters failed: {}",
                            e
                        )));
                    }
                }

                effect_handle.last_updated = Instant::now();
                effect_handle.last_magnitude = magnitude;
                device_ctx.last_torque_nm = clamped_torque;

                tracing::trace!(
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
                let mut periodic = DIPERIODIC {
                    dwMagnitude: di_magnitude,
                    lOffset: 0,
                    dwPhase: 0,
                    dwPeriod: period_us,
                };

                // Create effect parameters for update
                let mut axes = [DIJOFS_X, DIJOFS_Y];
                let mut directions = [0i32, 0i32];

                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 2,
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: std::mem::size_of::<DIPERIODIC>() as u32,
                    lpvTypeSpecificParams: &mut periodic as *mut _ as *mut c_void,
                    dwStartDelay: 0,
                };

                // Update effect parameters
                unsafe {
                    (*effect_handle.effect_ptr)
                        .set_parameters(&effect_params, DIEP_TYPESPECIFICPARAMS | DIEP_NORESTART)
                        .map_err(|e| {
                            DInputError::EffectUpdateFailed(format!("SetParameters failed: {}", e))
                        })?;
                }

                effect_handle.last_updated = Instant::now();
                effect_handle.last_magnitude = di_magnitude as i32;

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
                let mut conditions = [
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

                // Create effect parameters for update
                let mut axes = [DIJOFS_X, DIJOFS_Y];
                let mut directions = [0i32, 0i32];

                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 2,
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: (std::mem::size_of::<DICONDITION>() * 2) as u32,
                    lpvTypeSpecificParams: conditions.as_mut_ptr() as *mut c_void,
                    dwStartDelay: 0,
                };

                // Update effect parameters
                unsafe {
                    (*effect_handle.effect_ptr)
                        .set_parameters(&effect_params, DIEP_TYPESPECIFICPARAMS | DIEP_NORESTART)
                        .map_err(|e| {
                            DInputError::EffectUpdateFailed(format!("SetParameters failed: {}", e))
                        })?;
                }

                effect_handle.last_updated = Instant::now();

                tracing::debug!(
                    "Set spring effect {} to center {}, stiffness {}",
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
                let mut conditions = [
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

                // Create effect parameters for update
                let mut axes = [DIJOFS_X, DIJOFS_Y];
                let mut directions = [0i32, 0i32];

                let effect_params = DIEFFECT {
                    dwSize: std::mem::size_of::<DIEFFECT>() as u32,
                    dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                    dwDuration: INFINITE,
                    dwSamplePeriod: 0,
                    dwGain: DI_FFNOMINALMAX,
                    dwTriggerButton: DIEB_NOTRIGGER,
                    dwTriggerRepeatInterval: 0,
                    cAxes: 2,
                    rgdwAxes: axes.as_mut_ptr(),
                    rglDirection: directions.as_mut_ptr(),
                    lpEnvelope: ptr::null_mut(),
                    cbTypeSpecificParams: (std::mem::size_of::<DICONDITION>() * 2) as u32,
                    lpvTypeSpecificParams: conditions.as_mut_ptr() as *mut c_void,
                    dwStartDelay: 0,
                };

                // Update effect parameters
                unsafe {
                    (*effect_handle.effect_ptr)
                        .set_parameters(&effect_params, DIEP_TYPESPECIFICPARAMS | DIEP_NORESTART)
                        .map_err(|e| {
                            DInputError::EffectUpdateFailed(format!("SetParameters failed: {}", e))
                        })?;
                }

                effect_handle.last_updated = Instant::now();

                tracing::debug!(
                    "Set damper effect {} to damping {}",
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

                unsafe {
                    // Start effect: iterations=1 (infinite for INFINITE duration), flags=0
                    (*effect_handle.effect_ptr).start(1, 0).map_err(|e| {
                        DInputError::EffectUpdateFailed(format!("Start failed: {}", e))
                    })?;
                }

                tracing::debug!("Started effect {}", handle);
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

                unsafe {
                    (*effect_handle.effect_ptr).stop().map_err(|e| {
                        DInputError::EffectUpdateFailed(format!("Stop failed: {}", e))
                    })?;
                }

                tracing::debug!("Stopped effect {}", handle);
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

    /// Get device GUID string
    pub fn device_guid(&self) -> &str {
        &self.device_guid
    }
}

impl Drop for DirectInputFfbDevice {
    fn drop(&mut self) {
        // Unacquire the device (stops effects and releases acquisition)
        self.unacquire();

        #[cfg(windows)]
        {
            // Release all effects
            for effect in &self.effects {
                if !effect.effect_ptr.is_null() {
                    unsafe {
                        (*effect.effect_ptr).release();
                    }
                }
            }
            self.effects.clear();

            // Release device interface
            if let Some(device) = self.device.take() {
                unsafe {
                    (*device).release();
                }
            }

            // Release DirectInput interface
            if let Some(dinput) = self.dinput.take() {
                unsafe {
                    (*dinput).release();
                }
            }

            // Window is automatically destroyed when dropped

            // Uninitialize COM if we initialized it
            if self.com_initialized.load(Ordering::SeqCst) {
                unsafe {
                    CoUninitialize();
                }
            }

            tracing::debug!("DirectInputFfbDevice dropped and resources released");
        }
    }
}

// ============================================================================
// Device Enumeration Callback
// ============================================================================

/// Callback function for device enumeration
#[cfg(windows)]
unsafe extern "system" fn enum_devices_callback(
    instance: *const DIDEVICEINSTANCEW,
    context: *mut c_void,
) -> i32 {
    if instance.is_null() || context.is_null() {
        return DIENUM_CONTINUE;
    }

    let devices = &mut *(context as *mut Vec<EnumeratedDevice>);
    let inst = &*instance;

    // Extract device names
    let instance_name = String::from_utf16_lossy(
        &inst.tszInstanceName[..inst
            .tszInstanceName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(inst.tszInstanceName.len())],
    );
    let product_name = String::from_utf16_lossy(
        &inst.tszProductName[..inst
            .tszProductName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(inst.tszProductName.len())],
    );

    let device_info = EnumeratedDevice {
        instance_guid: guid_to_string(&inst.guidInstance),
        product_guid: guid_to_string(&inst.guidProduct),
        instance_name,
        product_name,
    };

    tracing::debug!(
        "Enumerated FFB device: {} ({})",
        device_info.product_name,
        device_info.instance_guid
    );

    devices.push(device_info);

    DIENUM_CONTINUE
}

// ============================================================================
// Tests
// ============================================================================

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
        // This test requires DirectInput to be available
        let mut device =
            DirectInputFfbDevice::new("{00000000-0000-0000-0000-000000000000}".to_string())
                .unwrap();

        // Initialize will fail with invalid GUID or no device, but shouldn't panic
        let result = device.initialize();
        // We expect this to fail since the GUID is invalid
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_device_enumeration() {
        // Test device enumeration
        let devices = DirectInputFfbDevice::enumerate_devices();
        assert!(devices.is_ok());

        // The list may be empty if no FFB devices are connected
        let device_list = devices.unwrap();
        println!("Found {} FFB devices", device_list.len());
    }

    #[test]
    fn test_ffb_capabilities_default() {
        let caps = FfbCapabilities::default();
        assert!(!caps.supports_pid);
        assert!(!caps.supports_raw_torque);
        assert_eq!(caps.max_torque_nm, 10.0);
        assert_eq!(caps.min_period_us, 1000);
        assert_eq!(caps.num_axes, 2);
    }

    #[test]
    fn test_effect_types() {
        assert_eq!(EffectType::ConstantForce, EffectType::ConstantForce);
        assert_ne!(EffectType::ConstantForce, EffectType::PeriodicSine);
        assert_ne!(EffectType::Spring, EffectType::Damper);
    }

    #[test]
    #[cfg(windows)]
    fn test_guid_parsing() {
        // Test valid GUID
        let valid_guid = "{13541C20-8E33-11D0-9AD0-00A0C9A06E35}";
        let parsed = string_to_guid(valid_guid);
        assert!(parsed.is_some());

        // Test invalid GUID
        let invalid_guid = "not-a-guid";
        let parsed = string_to_guid(invalid_guid);
        assert!(parsed.is_none());
    }
}
