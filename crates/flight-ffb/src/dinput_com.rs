// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DirectInput 8 COM interface bindings for force feedback
//!
//! This module provides Rust bindings for DirectInput 8 interfaces using dynamic
//! loading of dinput8.dll. DirectInput 8 is a legacy API not included in the modern
//! Windows SDK, so we use `LoadLibraryW`/`GetProcAddress` to access it at runtime.
//!
//! # Safety
//!
//! All COM interface methods are unsafe as they involve raw pointer manipulation
//! and FFI calls. The higher-level abstractions in `dinput_device.rs` provide
//! safe wrappers around these interfaces.

#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::upper_case_acronyms)]
// Allow unsafe operations in unsafe fns for this low-level COM bindings module
#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::mem::MaybeUninit;
#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
use windows::core::{Error, GUID, HRESULT, PCWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::{HMODULE, HWND};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

// ============================================================================
// DirectInput Version and Constants
// ============================================================================

/// DirectInput 8 version identifier
pub const DIRECTINPUT_VERSION: u32 = 0x0800;

/// Infinite duration for effects
pub const INFINITE: u32 = 0xFFFFFFFF;

/// Nominal maximum for DirectInput FFB values (-10000 to +10000)
pub const DI_FFNOMINALMAX: u32 = 10000;

/// No trigger button
pub const DIEB_NOTRIGGER: u32 = 0xFFFFFFFF;

// DIEFFECT flags
/// Use Cartesian coordinates for direction
pub const DIEFF_CARTESIAN: u32 = 0x00000010;
/// Offsets are object offsets (not object IDs)
pub const DIEFF_OBJECTOFFSETS: u32 = 0x00000002;

// DIEP flags for SetParameters
/// Update type-specific parameters
pub const DIEP_TYPESPECIFICPARAMS: u32 = 0x00000020;
/// Start the effect immediately
pub const DIEP_START: u32 = 0x20000000;
/// Don't restart if already playing
pub const DIEP_NORESTART: u32 = 0x40000000;
/// Update duration
pub const DIEP_DURATION: u32 = 0x00000001;
/// Update gain
pub const DIEP_GAIN: u32 = 0x00000004;
/// Update direction
pub const DIEP_DIRECTION: u32 = 0x00000040;

// SetCooperativeLevel flags
/// Exclusive access to device
pub const DISCL_EXCLUSIVE: u32 = 0x00000001;
/// Non-exclusive access
pub const DISCL_NONEXCLUSIVE: u32 = 0x00000002;
/// Foreground access only
pub const DISCL_FOREGROUND: u32 = 0x00000004;
/// Background access allowed
pub const DISCL_BACKGROUND: u32 = 0x00000008;
/// Disable Windows key
pub const DISCL_NOWINKEY: u32 = 0x00000010;

// Device class for enumeration
/// Game controller device class
pub const DI8DEVCLASS_GAMECTRL: u32 = 4;

// EnumDevices flags
/// Only enumerate attached devices
pub const DIEDFL_ATTACHEDONLY: u32 = 0x00000001;
/// Only enumerate force feedback devices
pub const DIEDFL_FORCEFEEDBACK: u32 = 0x00000100;

// Device capability flags
/// Device supports force feedback
pub const DIDC_FORCEFEEDBACK: u32 = 0x00000100;
/// FFB is actuated (motors connected)
pub const DIDC_FFCLIENT: u32 = 0x00002000;

// Joystick object offsets (for axis selection in effects)
/// X axis offset in data format
pub const DIJOFS_X: u32 = 0;
/// Y axis offset in data format
pub const DIJOFS_Y: u32 = 4;
/// Z axis offset in data format
pub const DIJOFS_Z: u32 = 8;
/// RX (rotation X) axis offset
pub const DIJOFS_RX: u32 = 12;
/// RY (rotation Y) axis offset
pub const DIJOFS_RY: u32 = 16;
/// RZ (rotation Z) axis offset
pub const DIJOFS_RZ: u32 = 20;

// Enumeration callback return values
/// Continue enumeration
pub const DIENUM_CONTINUE: i32 = 1;
/// Stop enumeration
pub const DIENUM_STOP: i32 = 0;

// ============================================================================
// DirectInput GUIDs
// ============================================================================

/// IID_IDirectInput8W interface GUID
pub const IID_IDirectInput8W: GUID = GUID::from_u128(0xBF798031_483A_4DA2_AA99_5D64ED369700);

/// GUID for constant force effect
pub const GUID_ConstantForce: GUID = GUID::from_u128(0x13541C20_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for sine wave periodic effect
pub const GUID_Sine: GUID = GUID::from_u128(0x13541C23_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for square wave periodic effect
pub const GUID_Square: GUID = GUID::from_u128(0x13541C22_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for triangle wave periodic effect
pub const GUID_Triangle: GUID = GUID::from_u128(0x13541C24_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for sawtooth up periodic effect
pub const GUID_SawtoothUp: GUID = GUID::from_u128(0x13541C25_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for sawtooth down periodic effect
pub const GUID_SawtoothDown: GUID = GUID::from_u128(0x13541C26_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for spring condition effect
pub const GUID_Spring: GUID = GUID::from_u128(0x13541C27_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for damper condition effect
pub const GUID_Damper: GUID = GUID::from_u128(0x13541C28_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for inertia condition effect
pub const GUID_Inertia: GUID = GUID::from_u128(0x13541C29_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for friction condition effect
pub const GUID_Friction: GUID = GUID::from_u128(0x13541C2A_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for ramp force effect
pub const GUID_RampForce: GUID = GUID::from_u128(0x13541C21_8E33_11D0_9AD0_00A0C9A06E35);

/// GUID for custom force effect
pub const GUID_CustomForce: GUID = GUID::from_u128(0x13541C2B_8E33_11D0_9AD0_00A0C9A06E35);

// Joystick data format GUID
/// GUID for standard joystick data format (c_dfDIJoystick2)
pub const GUID_Joystick: GUID = GUID::from_u128(0x6F1D2B70_D5A0_11CF_BFC7_444553540000);

// ============================================================================
// DirectInput Error Codes (HRESULT values)
// ============================================================================

/// Device input lost (usually means device was unplugged)
pub const DIERR_INPUTLOST: i32 = 0x8007001E_u32 as i32;

/// Device not acquired
pub const DIERR_NOTACQUIRED: i32 = 0x8007000C_u32 as i32;

/// Device has been unplugged
pub const DIERR_UNPLUGGED: i32 = 0x80040209_u32 as i32;

/// Effect not supported by device
pub const DIERR_UNSUPPORTED: i32 = 0x80004001_u32 as i32;

/// Invalid parameter
pub const DIERR_INVALIDPARAM: i32 = 0x80070057_u32 as i32;

/// Out of memory
pub const DIERR_OUTOFMEMORY: i32 = 0x8007000E_u32 as i32;

/// Effect is playing
pub const DIERR_EFFECTPLAYING: i32 = 0x80040207_u32 as i32;

/// Device is not initialized
pub const DIERR_NOTINITIALIZED: i32 = 0x80070015_u32 as i32;

/// Success
pub const DI_OK: i32 = 0;

/// Success, but some parameters were modified
pub const DI_TRUNCATED: i32 = 0x00000008;

// ============================================================================
// DirectInput Structures
// ============================================================================

/// Constant force effect parameters
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DICONSTANTFORCE {
    /// Magnitude of constant force (-10000 to +10000)
    pub lMagnitude: i32,
}

/// Periodic effect parameters
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DIPERIODIC {
    /// Peak value (0 to 10000)
    pub dwMagnitude: u32,
    /// Mean value of wave (-10000 to +10000)
    pub lOffset: i32,
    /// Phase of waveform (0 to 35999, in hundredths of degrees)
    pub dwPhase: u32,
    /// Period of waveform in microseconds
    pub dwPeriod: u32,
}

/// Condition effect parameters (spring, damper, friction, inertia)
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DICONDITION {
    /// Offset from center (-10000 to +10000)
    pub lOffset: i32,
    /// Coefficient for positive displacement (-10000 to +10000)
    pub lPositiveCoefficient: i32,
    /// Coefficient for negative displacement (-10000 to +10000)
    pub lNegativeCoefficient: i32,
    /// Maximum force in positive direction (0 to 10000)
    pub dwPositiveSaturation: u32,
    /// Maximum force in negative direction (0 to 10000)
    pub dwNegativeSaturation: u32,
    /// Size of dead band (-10000 to +10000)
    pub lDeadBand: i32,
}

/// Ramp force effect parameters
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DIRAMPFORCE {
    /// Start magnitude (-10000 to +10000)
    pub lStart: i32,
    /// End magnitude (-10000 to +10000)
    pub lEnd: i32,
}

/// Effect envelope parameters
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DIENVELOPE {
    /// Size of this structure
    pub dwSize: u32,
    /// Attack level (0 to 10000)
    pub dwAttackLevel: u32,
    /// Attack time in microseconds
    pub dwAttackTime: u32,
    /// Fade level (0 to 10000)
    pub dwFadeLevel: u32,
    /// Fade time in microseconds
    pub dwFadeTime: u32,
}

#[cfg(windows)]
impl DIENVELOPE {
    pub fn new() -> Self {
        Self {
            dwSize: std::mem::size_of::<Self>() as u32,
            ..Default::default()
        }
    }
}

/// Effect parameters structure
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DIEFFECT {
    /// Size of this structure
    pub dwSize: u32,
    /// Flags (DIEFF_*)
    pub dwFlags: u32,
    /// Duration in microseconds (INFINITE for infinite)
    pub dwDuration: u32,
    /// Sample period in microseconds (0 for default)
    pub dwSamplePeriod: u32,
    /// Effect gain (0 to 10000)
    pub dwGain: u32,
    /// Trigger button (DIEB_NOTRIGGER for none)
    pub dwTriggerButton: u32,
    /// Repeat interval for triggered effects in microseconds
    pub dwTriggerRepeatInterval: u32,
    /// Number of axes
    pub cAxes: u32,
    /// Pointer to array of axis offsets (DIJOFS_*)
    pub rgdwAxes: *mut u32,
    /// Pointer to array of direction values
    pub rglDirection: *mut i32,
    /// Pointer to envelope (NULL for no envelope)
    pub lpEnvelope: *mut DIENVELOPE,
    /// Size of type-specific parameters
    pub cbTypeSpecificParams: u32,
    /// Pointer to type-specific parameters
    pub lpvTypeSpecificParams: *mut c_void,
    /// Start delay in microseconds
    pub dwStartDelay: u32,
}

#[cfg(windows)]
impl Default for DIEFFECT {
    fn default() -> Self {
        Self {
            dwSize: std::mem::size_of::<Self>() as u32,
            dwFlags: 0,
            dwDuration: 0,
            dwSamplePeriod: 0,
            dwGain: DI_FFNOMINALMAX,
            dwTriggerButton: DIEB_NOTRIGGER,
            dwTriggerRepeatInterval: 0,
            cAxes: 0,
            rgdwAxes: ptr::null_mut(),
            rglDirection: ptr::null_mut(),
            lpEnvelope: ptr::null_mut(),
            cbTypeSpecificParams: 0,
            lpvTypeSpecificParams: ptr::null_mut(),
            dwStartDelay: 0,
        }
    }
}

/// Device instance information returned by EnumDevices
#[cfg(windows)]
#[repr(C)]
#[derive(Clone)]
pub struct DIDEVICEINSTANCEW {
    /// Size of this structure
    pub dwSize: u32,
    /// Device instance GUID
    pub guidInstance: GUID,
    /// Device product GUID
    pub guidProduct: GUID,
    /// Device type
    pub dwDevType: u32,
    /// Instance name (wide string)
    pub tszInstanceName: [u16; 260],
    /// Product name (wide string)
    pub tszProductName: [u16; 260],
    /// FFB driver GUID
    pub guidFFDriver: GUID,
    /// Usage page (HID)
    pub wUsagePage: u16,
    /// Usage (HID)
    pub wUsage: u16,
}

#[cfg(windows)]
impl Default for DIDEVICEINSTANCEW {
    fn default() -> Self {
        Self {
            dwSize: std::mem::size_of::<Self>() as u32,
            guidInstance: GUID::zeroed(),
            guidProduct: GUID::zeroed(),
            dwDevType: 0,
            tszInstanceName: [0u16; 260],
            tszProductName: [0u16; 260],
            guidFFDriver: GUID::zeroed(),
            wUsagePage: 0,
            wUsage: 0,
        }
    }
}

#[cfg(windows)]
impl std::fmt::Debug for DIDEVICEINSTANCEW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let instance_name = String::from_utf16_lossy(
            &self.tszInstanceName[..self
                .tszInstanceName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(self.tszInstanceName.len())],
        );
        let product_name = String::from_utf16_lossy(
            &self.tszProductName[..self
                .tszProductName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(self.tszProductName.len())],
        );
        f.debug_struct("DIDEVICEINSTANCEW")
            .field("guidInstance", &self.guidInstance)
            .field("guidProduct", &self.guidProduct)
            .field("dwDevType", &self.dwDevType)
            .field("tszInstanceName", &instance_name)
            .field("tszProductName", &product_name)
            .finish()
    }
}

/// Device capabilities
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct DIDEVCAPS {
    /// Size of this structure
    pub dwSize: u32,
    /// Capability flags (DIDC_*)
    pub dwFlags: u32,
    /// Device type
    pub dwDevType: u32,
    /// Number of axes
    pub dwAxes: u32,
    /// Number of buttons
    pub dwButtons: u32,
    /// Number of POV controllers
    pub dwPOVs: u32,
    /// Size of input data in bytes (when polled)
    pub dwFFSamplePeriod: u32,
    /// Minimum time resolution for timed effects
    pub dwFFMinTimeResolution: u32,
    /// Size of the device state in bytes
    pub dwFirmwareRevision: u32,
    /// Hardware revision
    pub dwHardwareRevision: u32,
    /// Driver version
    pub dwFFDriverVersion: u32,
}

#[cfg(windows)]
impl DIDEVCAPS {
    pub fn new() -> Self {
        Self {
            dwSize: std::mem::size_of::<Self>() as u32,
            ..Default::default()
        }
    }
}

/// Effect information returned by EnumEffects
#[cfg(windows)]
#[repr(C)]
#[derive(Clone)]
pub struct DIEFFECTINFOW {
    /// Size of this structure
    pub dwSize: u32,
    /// Effect type GUID
    pub guid: GUID,
    /// Effect type flags
    pub dwEffType: u32,
    /// Dynamic parameters supported
    pub dwStaticParams: u32,
    /// Dynamic parameters supported
    pub dwDynamicParams: u32,
    /// Effect name (wide string)
    pub tszName: [u16; 260],
}

#[cfg(windows)]
impl Default for DIEFFECTINFOW {
    fn default() -> Self {
        Self {
            dwSize: std::mem::size_of::<Self>() as u32,
            guid: GUID::zeroed(),
            dwEffType: 0,
            dwStaticParams: 0,
            dwDynamicParams: 0,
            tszName: [0u16; 260],
        }
    }
}

// ============================================================================
// COM Interface VTable Definitions
// ============================================================================

/// IDirectInputEffect interface VTable
#[cfg(windows)]
#[repr(C)]
pub struct IDirectInputEffectVtbl {
    // IUnknown methods
    pub QueryInterface:
        unsafe extern "system" fn(*mut IDirectInputEffect, *const GUID, *mut *mut c_void) -> i32,
    pub AddRef: unsafe extern "system" fn(*mut IDirectInputEffect) -> u32,
    pub Release: unsafe extern "system" fn(*mut IDirectInputEffect) -> u32,

    // IDirectInputEffect methods
    pub Initialize: unsafe extern "system" fn(
        *mut IDirectInputEffect,
        HMODULE,
        u32,
        *const GUID,
    ) -> i32,
    pub GetEffectGuid: unsafe extern "system" fn(*mut IDirectInputEffect, *mut GUID) -> i32,
    pub GetParameters:
        unsafe extern "system" fn(*mut IDirectInputEffect, *mut DIEFFECT, u32) -> i32,
    pub SetParameters:
        unsafe extern "system" fn(*mut IDirectInputEffect, *const DIEFFECT, u32) -> i32,
    pub Start: unsafe extern "system" fn(*mut IDirectInputEffect, u32, u32) -> i32,
    pub Stop: unsafe extern "system" fn(*mut IDirectInputEffect) -> i32,
    pub GetEffectStatus: unsafe extern "system" fn(*mut IDirectInputEffect, *mut u32) -> i32,
    pub Download: unsafe extern "system" fn(*mut IDirectInputEffect) -> i32,
    pub Unload: unsafe extern "system" fn(*mut IDirectInputEffect) -> i32,
    pub Escape: unsafe extern "system" fn(*mut IDirectInputEffect, *mut c_void) -> i32,
}

/// IDirectInputEffect interface
#[cfg(windows)]
#[repr(C)]
pub struct IDirectInputEffect {
    pub lpVtbl: *const IDirectInputEffectVtbl,
}

#[cfg(windows)]
impl IDirectInputEffect {
    /// Set effect parameters
    ///
    /// # Safety
    /// Caller must ensure `params` is valid and properly initialized.
    pub unsafe fn set_parameters(&self, params: &DIEFFECT, flags: u32) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).SetParameters)(self as *const _ as *mut _, params, flags);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Start the effect
    ///
    /// # Safety
    /// Caller must ensure the effect has been properly created and downloaded.
    pub unsafe fn start(&self, iterations: u32, flags: u32) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).Start)(self as *const _ as *mut _, iterations, flags);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Stop the effect
    ///
    /// # Safety
    /// Caller must ensure the effect is valid.
    pub unsafe fn stop(&self) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).Stop)(self as *const _ as *mut _);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Release the effect
    ///
    /// # Safety
    /// Caller must ensure no other references to this effect exist.
    pub unsafe fn release(&self) -> u32 {
        ((*self.lpVtbl).Release)(self as *const _ as *mut _)
    }

    /// Download the effect to the device
    ///
    /// # Safety
    /// Caller must ensure the effect parameters have been set.
    pub unsafe fn download(&self) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).Download)(self as *const _ as *mut _);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Get effect status
    ///
    /// # Safety
    /// Caller must provide a valid pointer for the status.
    pub unsafe fn get_effect_status(&self) -> Result<u32, Error> {
        let mut status: u32 = 0;
        let hr = ((*self.lpVtbl).GetEffectStatus)(self as *const _ as *mut _, &mut status);
        if hr >= 0 {
            Ok(status)
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }
}

/// IDirectInputDevice8W interface VTable
#[cfg(windows)]
#[repr(C)]
pub struct IDirectInputDevice8WVtbl {
    // IUnknown methods
    pub QueryInterface: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const GUID,
        *mut *mut c_void,
    ) -> i32,
    pub AddRef: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> u32,
    pub Release: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> u32,

    // IDirectInputDevice8W methods
    pub GetCapabilities:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut DIDEVCAPS) -> i32,
    pub EnumObjects: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const c_void,
        *mut c_void,
        u32,
    ) -> i32,
    pub GetProperty:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *const GUID, *mut c_void) -> i32,
    pub SetProperty:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *const GUID, *const c_void) -> i32,
    pub Acquire: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> i32,
    pub Unacquire: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> i32,
    pub GetDeviceState: unsafe extern "system" fn(*mut IDirectInputDevice8W, u32, *mut c_void) -> i32,
    pub GetDeviceData: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        u32,
        *mut c_void,
        *mut u32,
        u32,
    ) -> i32,
    pub SetDataFormat:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *const c_void) -> i32,
    pub SetEventNotification:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *const c_void) -> i32,
    pub SetCooperativeLevel:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, HWND, u32) -> i32,
    pub GetObjectInfo:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut c_void, u32, u32) -> i32,
    pub GetDeviceInfo:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut DIDEVICEINSTANCEW) -> i32,
    pub RunControlPanel:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, HWND, u32) -> i32,
    pub Initialize:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, HMODULE, u32, *const GUID) -> i32,
    pub CreateEffect: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const GUID,
        *const DIEFFECT,
        *mut *mut IDirectInputEffect,
        *mut c_void,
    ) -> i32,
    pub EnumEffects: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const c_void,
        *mut c_void,
        u32,
    ) -> i32,
    pub GetEffectInfo:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut DIEFFECTINFOW, *const GUID) -> i32,
    pub GetForceFeedbackState:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut u32) -> i32,
    pub SendForceFeedbackCommand:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, u32) -> i32,
    pub EnumCreatedEffectObjects: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const c_void,
        *mut c_void,
        u32,
    ) -> i32,
    pub Escape: unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut c_void) -> i32,
    pub Poll: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> i32,
    pub SendDeviceData: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        u32,
        *const c_void,
        *mut u32,
        u32,
    ) -> i32,
    pub EnumEffectsInFile: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        PCWSTR,
        *const c_void,
        *mut c_void,
        u32,
    ) -> i32,
    pub WriteEffectToFile: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        PCWSTR,
        u32,
        *const c_void,
        u32,
    ) -> i32,
    pub BuildActionMap: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *mut c_void,
        PCWSTR,
        u32,
    ) -> i32,
    pub SetActionMap:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *const c_void, PCWSTR, u32) -> i32,
    pub GetImageInfo:
        unsafe extern "system" fn(*mut IDirectInputDevice8W, *mut c_void) -> i32,
}

/// IDirectInputDevice8W interface
#[cfg(windows)]
#[repr(C)]
pub struct IDirectInputDevice8W {
    pub lpVtbl: *const IDirectInputDevice8WVtbl,
}

#[cfg(windows)]
impl IDirectInputDevice8W {
    /// Get device capabilities
    ///
    /// # Safety
    /// Caller must provide a valid DIDEVCAPS structure.
    pub unsafe fn get_capabilities(&self, caps: &mut DIDEVCAPS) -> Result<(), Error> {
        caps.dwSize = std::mem::size_of::<DIDEVCAPS>() as u32;
        let hr = ((*self.lpVtbl).GetCapabilities)(self as *const _ as *mut _, caps);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Set cooperative level
    ///
    /// # Safety
    /// Caller must provide a valid HWND.
    pub unsafe fn set_cooperative_level(&self, hwnd: HWND, flags: u32) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).SetCooperativeLevel)(self as *const _ as *mut _, hwnd, flags);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Acquire the device
    ///
    /// # Safety
    /// Device must be properly initialized and cooperative level set.
    pub unsafe fn acquire(&self) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).Acquire)(self as *const _ as *mut _);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Unacquire the device
    ///
    /// # Safety
    /// Device must have been acquired.
    pub unsafe fn unacquire(&self) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).Unacquire)(self as *const _ as *mut _);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Set data format
    ///
    /// # Safety
    /// Caller must provide a valid data format.
    pub unsafe fn set_data_format(&self, format: *const c_void) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).SetDataFormat)(self as *const _ as *mut _, format);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Create an effect
    ///
    /// # Safety
    /// Caller must provide valid GUID and DIEFFECT.
    pub unsafe fn create_effect(
        &self,
        effect_guid: &GUID,
        params: &DIEFFECT,
    ) -> Result<*mut IDirectInputEffect, Error> {
        let mut effect: *mut IDirectInputEffect = ptr::null_mut();
        let hr = ((*self.lpVtbl).CreateEffect)(
            self as *const _ as *mut _,
            effect_guid,
            params,
            &mut effect,
            ptr::null_mut(),
        );
        if hr >= 0 {
            Ok(effect)
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Get device info
    ///
    /// # Safety
    /// Caller must provide a valid DIDEVICEINSTANCEW structure.
    pub unsafe fn get_device_info(&self, info: &mut DIDEVICEINSTANCEW) -> Result<(), Error> {
        info.dwSize = std::mem::size_of::<DIDEVICEINSTANCEW>() as u32;
        let hr = ((*self.lpVtbl).GetDeviceInfo)(self as *const _ as *mut _, info);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Poll the device
    ///
    /// # Safety
    /// Device must be acquired.
    pub unsafe fn poll(&self) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).Poll)(self as *const _ as *mut _);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Release the device
    ///
    /// # Safety
    /// Caller must ensure no other references to this device exist.
    pub unsafe fn release(&self) -> u32 {
        ((*self.lpVtbl).Release)(self as *const _ as *mut _)
    }

    /// Send force feedback command
    ///
    /// # Safety
    /// Device must be acquired and support force feedback.
    pub unsafe fn send_force_feedback_command(&self, command: u32) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).SendForceFeedbackCommand)(self as *const _ as *mut _, command);
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }
}

/// IDirectInput8W interface VTable
#[cfg(windows)]
#[repr(C)]
pub struct IDirectInput8WVtbl {
    // IUnknown methods
    pub QueryInterface:
        unsafe extern "system" fn(*mut IDirectInput8W, *const GUID, *mut *mut c_void) -> i32,
    pub AddRef: unsafe extern "system" fn(*mut IDirectInput8W) -> u32,
    pub Release: unsafe extern "system" fn(*mut IDirectInput8W) -> u32,

    // IDirectInput8W methods
    pub CreateDevice: unsafe extern "system" fn(
        *mut IDirectInput8W,
        *const GUID,
        *mut *mut IDirectInputDevice8W,
        *mut c_void,
    ) -> i32,
    pub EnumDevices: unsafe extern "system" fn(
        *mut IDirectInput8W,
        u32,
        unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> i32,
        *mut c_void,
        u32,
    ) -> i32,
    pub GetDeviceStatus: unsafe extern "system" fn(*mut IDirectInput8W, *const GUID) -> i32,
    pub RunControlPanel: unsafe extern "system" fn(*mut IDirectInput8W, HWND, u32) -> i32,
    pub Initialize: unsafe extern "system" fn(*mut IDirectInput8W, HMODULE, u32) -> i32,
    pub FindDevice:
        unsafe extern "system" fn(*mut IDirectInput8W, *const GUID, PCWSTR, *mut GUID) -> i32,
    pub EnumDevicesBySemantics: unsafe extern "system" fn(
        *mut IDirectInput8W,
        PCWSTR,
        *const c_void,
        *const c_void,
        *mut c_void,
        u32,
    ) -> i32,
    pub ConfigureDevices: unsafe extern "system" fn(
        *mut IDirectInput8W,
        *const c_void,
        *const c_void,
        u32,
        *mut c_void,
    ) -> i32,
}

/// IDirectInput8W interface
#[cfg(windows)]
#[repr(C)]
pub struct IDirectInput8W {
    pub lpVtbl: *const IDirectInput8WVtbl,
}

#[cfg(windows)]
impl IDirectInput8W {
    /// Create a device
    ///
    /// # Safety
    /// Caller must provide a valid device GUID.
    pub unsafe fn create_device(
        &self,
        device_guid: &GUID,
    ) -> Result<*mut IDirectInputDevice8W, Error> {
        let mut device: *mut IDirectInputDevice8W = ptr::null_mut();
        let hr = ((*self.lpVtbl).CreateDevice)(
            self as *const _ as *mut _,
            device_guid,
            &mut device,
            ptr::null_mut(),
        );
        if hr >= 0 {
            Ok(device)
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Enumerate devices
    ///
    /// # Safety
    /// Callback must be safe to call from DirectInput context.
    pub unsafe fn enum_devices(
        &self,
        device_class: u32,
        callback: unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> i32,
        context: *mut c_void,
        flags: u32,
    ) -> Result<(), Error> {
        let hr = ((*self.lpVtbl).EnumDevices)(
            self as *const _ as *mut _,
            device_class,
            callback,
            context,
            flags,
        );
        if hr >= 0 {
            Ok(())
        } else {
            Err(Error::from_hresult(HRESULT(hr)))
        }
    }

    /// Release the interface
    ///
    /// # Safety
    /// Caller must ensure no other references to this interface exist.
    pub unsafe fn release(&self) -> u32 {
        ((*self.lpVtbl).Release)(self as *const _ as *mut _)
    }
}

// ============================================================================
// DirectInput8Create Function Binding
// ============================================================================

/// Type for DirectInput8Create function pointer
#[cfg(windows)]
type DirectInput8CreateFn = unsafe extern "system" fn(
    hinst: HMODULE,
    dwVersion: u32,
    riidltf: *const GUID,
    ppvOut: *mut *mut c_void,
    punkOuter: *mut c_void,
) -> i32;

/// Wrapper for HMODULE that's Send + Sync (the module handle is valid process-wide)
#[cfg(windows)]
struct SyncModule(HMODULE);

#[cfg(windows)]
unsafe impl Send for SyncModule {}
#[cfg(windows)]
unsafe impl Sync for SyncModule {}

/// Cached handle to dinput8.dll and DirectInput8Create function
#[cfg(windows)]
static DINPUT8_DLL: OnceLock<Option<(SyncModule, DirectInput8CreateFn)>> = OnceLock::new();

/// Load dinput8.dll and get DirectInput8Create function
///
/// # Safety
/// This function is safe to call multiple times; it caches the result.
#[cfg(windows)]
fn load_dinput8() -> Option<(HMODULE, DirectInput8CreateFn)> {
    let cached = DINPUT8_DLL.get_or_init(|| unsafe {
        let dll_name: Vec<u16> = "dinput8.dll\0".encode_utf16().collect();
        let module = LoadLibraryW(PCWSTR::from_raw(dll_name.as_ptr())).ok()?;

        let proc_name = b"DirectInput8Create\0";
        let proc = GetProcAddress(module, windows::core::PCSTR::from_raw(proc_name.as_ptr()))?;

        let create_fn: DirectInput8CreateFn = std::mem::transmute(proc);

        Some((SyncModule(module), create_fn))
    });
    cached.as_ref().map(|(m, f)| (m.0, *f))
}

/// Create IDirectInput8W interface
///
/// # Safety
/// Caller must ensure COM has been initialized.
#[cfg(windows)]
pub unsafe fn create_direct_input8(hinst: HMODULE) -> Result<*mut IDirectInput8W, Error> {
    let (_, create_fn) = load_dinput8().ok_or_else(|| {
        Error::from_hresult(HRESULT(DIERR_NOTINITIALIZED))
    })?;

    let mut dinput: *mut c_void = ptr::null_mut();
    let hr = create_fn(
        hinst,
        DIRECTINPUT_VERSION,
        &IID_IDirectInput8W,
        &mut dinput,
        ptr::null_mut(),
    );

    if hr >= 0 {
        Ok(dinput as *mut IDirectInput8W)
    } else {
        Err(Error::from_hresult(HRESULT(hr)))
    }
}

/// Check if DirectInput8 is available on this system
#[cfg(windows)]
pub fn is_dinput8_available() -> bool {
    load_dinput8().is_some()
}

// ============================================================================
// Force Feedback Commands
// ============================================================================

/// Force feedback command: Reset device to neutral state
pub const DISFFC_RESET: u32 = 0x00000001;
/// Force feedback command: Stop all effects
pub const DISFFC_STOPALL: u32 = 0x00000002;
/// Force feedback command: Pause all effects
pub const DISFFC_PAUSE: u32 = 0x00000004;
/// Force feedback command: Resume paused effects
pub const DISFFC_CONTINUE: u32 = 0x00000008;
/// Force feedback command: Enable actuators
pub const DISFFC_SETACTUATORSON: u32 = 0x00000010;
/// Force feedback command: Disable actuators
pub const DISFFC_SETACTUATORSOFF: u32 = 0x00000020;

// ============================================================================
// Effect Status Flags
// ============================================================================

/// Effect is currently playing
pub const DIEGES_PLAYING: u32 = 0x00000001;
/// Effect is emulated (not native)
pub const DIEGES_EMULATED: u32 = 0x00000002;

// ============================================================================
// Joystick Data Format (c_dfDIJoystick2)
// ============================================================================

/// Standard joystick data format structure
/// This is a simplified version - the actual format is more complex
#[cfg(windows)]
#[repr(C)]
pub struct DIJOYSTATE2 {
    pub lX: i32,
    pub lY: i32,
    pub lZ: i32,
    pub lRx: i32,
    pub lRy: i32,
    pub lRz: i32,
    pub rglSlider: [i32; 2],
    pub rgdwPOV: [u32; 4],
    pub rgbButtons: [u8; 128],
    pub lVX: i32,
    pub lVY: i32,
    pub lVZ: i32,
    pub lVRx: i32,
    pub lVRy: i32,
    pub lVRz: i32,
    pub rglVSlider: [i32; 2],
    pub lAX: i32,
    pub lAY: i32,
    pub lAZ: i32,
    pub lARx: i32,
    pub lARy: i32,
    pub lARz: i32,
    pub rglASlider: [i32; 2],
    pub lFX: i32,
    pub lFY: i32,
    pub lFZ: i32,
    pub lFRx: i32,
    pub lFRy: i32,
    pub lFRz: i32,
    pub rglFSlider: [i32; 2],
}

#[cfg(windows)]
impl Default for DIJOYSTATE2 {
    fn default() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}

// ============================================================================
// Data Format Structures
// ============================================================================

/// Object data format for DIDATAFORMAT
#[cfg(windows)]
#[repr(C)]
pub struct DIOBJECTDATAFORMAT {
    pub pguid: *const GUID,
    pub dwOfs: u32,
    pub dwType: u32,
    pub dwFlags: u32,
}

/// Data format structure
#[cfg(windows)]
#[repr(C)]
pub struct DIDATAFORMAT {
    pub dwSize: u32,
    pub dwObjSize: u32,
    pub dwFlags: u32,
    pub dwDataSize: u32,
    pub dwNumObjs: u32,
    pub rgodf: *const DIOBJECTDATAFORMAT,
}

// Data format flags
pub const DIDF_ABSAXIS: u32 = 0x00000001;
pub const DIDF_RELAXIS: u32 = 0x00000002;

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a GUID to a string representation
#[cfg(windows)]
pub fn guid_to_string(guid: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7]
    )
}

/// Parse a GUID string into a GUID structure
#[cfg(windows)]
pub fn string_to_guid(s: &str) -> Option<GUID> {
    // Remove braces and split by dashes
    let s = s.trim_start_matches('{').trim_end_matches('}');
    let parts: Vec<&str> = s.split('-').collect();

    if parts.len() != 5 {
        return None;
    }

    let data1 = u32::from_str_radix(parts[0], 16).ok()?;
    let data2 = u16::from_str_radix(parts[1], 16).ok()?;
    let data3 = u16::from_str_radix(parts[2], 16).ok()?;

    let data4_0 = u8::from_str_radix(&parts[3][0..2], 16).ok()?;
    let data4_1 = u8::from_str_radix(&parts[3][2..4], 16).ok()?;

    let data4_2 = u8::from_str_radix(&parts[4][0..2], 16).ok()?;
    let data4_3 = u8::from_str_radix(&parts[4][2..4], 16).ok()?;
    let data4_4 = u8::from_str_radix(&parts[4][4..6], 16).ok()?;
    let data4_5 = u8::from_str_radix(&parts[4][6..8], 16).ok()?;
    let data4_6 = u8::from_str_radix(&parts[4][8..10], 16).ok()?;
    let data4_7 = u8::from_str_radix(&parts[4][10..12], 16).ok()?;

    Some(GUID {
        data1,
        data2,
        data3,
        data4: [
            data4_0, data4_1, data4_2, data4_3, data4_4, data4_5, data4_6, data4_7,
        ],
    })
}

/// Check if an HRESULT indicates a DirectInput error
#[cfg(windows)]
pub fn is_dinput_error(hr: i32) -> bool {
    hr < 0
}

/// Check if an HRESULT indicates device disconnection
#[cfg(windows)]
pub fn is_disconnect_error(hr: i32) -> bool {
    matches!(hr, DIERR_INPUTLOST | DIERR_UNPLUGGED | DIERR_NOTACQUIRED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guid_to_string() {
        #[cfg(windows)]
        {
            let guid_str = guid_to_string(&GUID_ConstantForce);
            assert!(guid_str.starts_with('{'));
            assert!(guid_str.ends_with('}'));
            assert_eq!(guid_str.len(), 38); // {xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}
        }
    }

    #[test]
    fn test_string_to_guid() {
        #[cfg(windows)]
        {
            let guid_str = "{13541C20-8E33-11D0-9AD0-00A0C9A06E35}";
            let guid = string_to_guid(guid_str);
            assert!(guid.is_some());
            let guid = guid.unwrap();
            assert_eq!(guid, GUID_ConstantForce);
        }
    }

    #[test]
    fn test_dieffect_default() {
        #[cfg(windows)]
        {
            let effect = DIEFFECT::default();
            assert_eq!(effect.dwSize, std::mem::size_of::<DIEFFECT>() as u32);
            assert_eq!(effect.dwGain, DI_FFNOMINALMAX);
            assert_eq!(effect.dwTriggerButton, DIEB_NOTRIGGER);
        }
    }

    #[test]
    fn test_didevcaps_new() {
        #[cfg(windows)]
        {
            let caps = DIDEVCAPS::new();
            assert_eq!(caps.dwSize, std::mem::size_of::<DIDEVCAPS>() as u32);
        }
    }

    #[test]
    fn test_is_disconnect_error() {
        #[cfg(windows)]
        {
            assert!(is_disconnect_error(DIERR_INPUTLOST));
            assert!(is_disconnect_error(DIERR_UNPLUGGED));
            assert!(is_disconnect_error(DIERR_NOTACQUIRED));
            assert!(!is_disconnect_error(DI_OK));
            assert!(!is_disconnect_error(DIERR_INVALIDPARAM));
        }
    }

    #[test]
    fn test_constants() {
        assert_eq!(DIRECTINPUT_VERSION, 0x0800);
        assert_eq!(DI_FFNOMINALMAX, 10000);
        assert_eq!(DIEB_NOTRIGGER, 0xFFFFFFFF);
    }
}
