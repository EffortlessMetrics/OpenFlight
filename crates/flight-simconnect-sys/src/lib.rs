#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(
    clippy::not_unsafe_ptr_arg_deref,
    clippy::missing_transmute_annotations
)]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Low-level FFI bindings for Microsoft SimConnect SDK
//!
//! This crate provides safe Rust bindings to the SimConnect API for Microsoft Flight Simulator.
//! It supports both dynamic linking (default) and static linking to the SimConnect library.
//!
//! # Features
//! - `dynamic` (default): Load SimConnect.dll at runtime
//! - `static`: Link against SimConnect.lib at compile time
//!
//! # Safety
//! All SimConnect API calls are wrapped in safe Rust interfaces with proper error handling.
//! The underlying FFI is marked as unsafe but exposed through safe abstractions.

use std::ffi::{c_char, c_void, CString};
use std::ptr;
use windows::core::{HRESULT, PCSTR, s};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::LibraryLoader::GetProcAddress;

/// SimConnect handle type
pub type HSIMCONNECT = *mut c_void;

/// SimConnect data definition ID
pub type SIMCONNECT_DATADEFID = u32;

/// SimConnect request ID
pub type SIMCONNECT_REQUESTID = u32;

/// SimConnect event ID
pub type SIMCONNECT_EVENTID = u32;

/// SimConnect client event ID
pub type SIMCONNECT_CLIENTEVENTID = u32;

/// SimConnect object ID
pub type SIMCONNECT_OBJECTID = u32;

/// SimConnect notification group ID
pub type SIMCONNECT_NOTIFICATIONGROUPID = u32;

/// SimConnect input group ID
pub type SIMCONNECT_INPUTGROUPID = u32;

/// SimConnect data type enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMCONNECT_DATATYPE {
    INVALID = 0,
    INT32 = 1,
    INT64 = 2,
    FLOAT32 = 3,
    FLOAT64 = 4,
    STRING8 = 5,
    STRING32 = 6,
    STRING64 = 7,
    STRING128 = 8,
    STRING256 = 9,
    STRING260 = 10,
    STRINGV = 11,
    INITPOSITION = 12,
    MARKERSTATE = 13,
    WAYPOINT = 14,
    LATLONALT = 15,
    XYZ = 16,
}

/// SimConnect period enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMCONNECT_PERIOD {
    NEVER = 0,
    ONCE = 1,
    VISUAL_FRAME = 2,
    SIM_FRAME = 3,
    SECOND = 4,
}

/// SimConnect client data period enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMCONNECT_CLIENT_DATA_PERIOD {
    NEVER = 0,
    ONCE = 1,
    VISUAL_FRAME = 2,
    ON_SET = 3,
}

/// SimConnect exception enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMCONNECT_EXCEPTION {
    NONE = 0,
    ERROR = 1,
    SIZE_MISMATCH = 2,
    UNRECOGNIZED_ID = 3,
    UNOPENED = 4,
    VERSION_MISMATCH = 5,
    TOO_MANY_GROUPS = 6,
    NAME_UNRECOGNIZED = 7,
    TOO_MANY_EVENT_NAMES = 8,
    EVENT_ID_DUPLICATE = 9,
    TOO_MANY_MAPS = 10,
    TOO_MANY_OBJECTS = 11,
    TOO_MANY_REQUESTS = 12,
    WEATHER_INVALID_PORT = 13,
    WEATHER_INVALID_METAR = 14,
    WEATHER_UNABLE_TO_GET_OBSERVATION = 15,
    WEATHER_UNABLE_TO_CREATE_STATION = 16,
    WEATHER_UNABLE_TO_REMOVE_STATION = 17,
    INVALID_DATA_TYPE = 18,
    INVALID_DATA_SIZE = 19,
    DATA_ERROR = 20,
    INVALID_ARRAY = 21,
    CREATE_OBJECT_FAILED = 22,
    LOAD_FLIGHTPLAN_FAILED = 23,
    OPERATION_INVALID_FOR_OBJECT_TYPE = 24,
    ILLEGAL_OPERATION = 25,
    ALREADY_SUBSCRIBED = 26,
    INVALID_ENUM = 27,
    DEFINITION_ERROR = 28,
    DUPLICATE_ID = 29,
    DATUM_ID = 30,
    OUT_OF_BOUNDS = 31,
    ALREADY_CREATED = 32,
    OBJECT_OUTSIDE_REALITY_BUBBLE = 33,
    OBJECT_CONTAINER = 34,
    OBJECT_AI = 35,
    OBJECT_ATC = 36,
    OBJECT_SCHEDULE = 37,
}

/// SimConnect receive ID enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMCONNECT_RECV_ID {
    NULL = 0,
    EXCEPTION = 1,
    OPEN = 2,
    QUIT = 3,
    EVENT = 4,
    EVENT_OBJECT_ADDREMOVE = 5,
    EVENT_FILENAME = 6,
    EVENT_FRAME = 7,
    SIMOBJECT_DATA = 8,
    SIMOBJECT_DATA_BYTYPE = 9,
    WEATHER_OBSERVATION = 10,
    CLOUD_STATE = 11,
    ASSIGNED_OBJECT_ID = 12,
    RESERVED_KEY = 13,
    CUSTOM_ACTION = 14,
    SYSTEM_STATE = 15,
    CLIENT_DATA = 16,
    EVENT_WEATHER_MODE = 17,
    AIRPORT_LIST = 18,
    VOR_LIST = 19,
    NDB_LIST = 20,
    WAYPOINT_LIST = 21,
    EVENT_MULTIPLAYER_SERVER_STARTED = 22,
    EVENT_MULTIPLAYER_CLIENT_STARTED = 23,
    EVENT_MULTIPLAYER_SESSION_ENDED = 24,
    EVENT_RACE_END = 25,
    EVENT_RACE_LAP = 26,
}

/// SimConnect receive structure header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SIMCONNECT_RECV {
    pub dwSize: u32,
    pub dwVersion: u32,
    pub dwID: u32,
}

/// SimConnect exception receive structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SIMCONNECT_RECV_EXCEPTION {
    pub hdr: SIMCONNECT_RECV,
    pub dwException: u32,
    pub dwSendID: u32,
    pub dwIndex: u32,
}

/// SimConnect open receive structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SIMCONNECT_RECV_OPEN {
    pub hdr: SIMCONNECT_RECV,
    pub szApplicationName: [c_char; 256],
    pub dwApplicationVersionMajor: u32,
    pub dwApplicationVersionMinor: u32,
    pub dwApplicationBuildMajor: u32,
    pub dwApplicationBuildMinor: u32,
    pub dwSimConnectVersionMajor: u32,
    pub dwSimConnectVersionMinor: u32,
    pub dwSimConnectBuildMajor: u32,
    pub dwSimConnectBuildMinor: u32,
    pub dwReserved1: u32,
    pub dwReserved2: u32,
}

/// SimConnect event receive structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SIMCONNECT_RECV_EVENT {
    pub hdr: SIMCONNECT_RECV,
    pub uGroupID: u32,
    pub uEventID: u32,
    pub dwData: u32,
}

/// SimConnect simobject data receive structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SIMCONNECT_RECV_SIMOBJECT_DATA {
    pub hdr: SIMCONNECT_RECV,
    pub dwRequestID: u32,
    pub dwObjectID: u32,
    pub dwDefineID: u32,
    pub dwFlags: u32,
    pub dwentrynumber: u32,
    pub dwoutof: u32,
    pub dwDefineCount: u32,
    // Data follows this structure
}

/// Function pointer types for dynamic loading
type SimConnect_Open = unsafe extern "system" fn(
    phSimConnect: *mut HSIMCONNECT,
    szName: PCSTR,
    hWnd: HWND,
    UserEventWin32: u32,
    hEventHandle: HANDLE,
    ConfigIndex: u32,
) -> HRESULT;

type SimConnect_Close = unsafe extern "system" fn(hSimConnect: HSIMCONNECT) -> HRESULT;

type SimConnect_CallDispatch =
    unsafe extern "system" fn(hSimConnect: HSIMCONNECT, pCallback: *mut c_void, pContext: *mut c_void) -> HRESULT;

type SimConnect_GetNextDispatch = unsafe extern "system" fn(
    hSimConnect: HSIMCONNECT,
    ppData: *mut *mut SIMCONNECT_RECV,
    pcbData: *mut u32,
) -> HRESULT;

type SimConnect_AddToDataDefinition = unsafe extern "system" fn(
    hSimConnect: HSIMCONNECT,
    DefineID: SIMCONNECT_DATADEFID,
    DatumName: PCSTR,
    UnitsName: PCSTR,
    DatumType: SIMCONNECT_DATATYPE,
    fEpsilon: f32,
    DatumID: u32,
) -> HRESULT;

type SimConnect_RequestDataOnSimObject = unsafe extern "system" fn(
    hSimConnect: HSIMCONNECT,
    RequestID: SIMCONNECT_REQUESTID,
    DefineID: SIMCONNECT_DATADEFID,
    ObjectID: SIMCONNECT_OBJECTID,
    Period: SIMCONNECT_PERIOD,
    Flags: u32,
    origin: u32,
    interval: u32,
    limit: u32,
) -> HRESULT;

type SimConnect_MapClientEventToSimEvent =
    unsafe extern "system" fn(hSimConnect: HSIMCONNECT, EventID: SIMCONNECT_EVENTID, EventName: PCSTR) -> HRESULT;

type SimConnect_TransmitClientEvent = unsafe extern "system" fn(
    hSimConnect: HSIMCONNECT,
    ObjectID: SIMCONNECT_OBJECTID,
    EventID: SIMCONNECT_EVENTID,
    dwData: u32,
    GroupID: SIMCONNECT_NOTIFICATIONGROUPID,
    Flags: u32,
) -> HRESULT;

type SimConnect_SubscribeToSystemEvent =
    unsafe extern "system" fn(hSimConnect: HSIMCONNECT, EventID: SIMCONNECT_EVENTID, SystemEventName: PCSTR) -> HRESULT;

/// SimConnect API wrapper with dynamic loading support
pub struct SimConnectApi {
    #[cfg(feature = "dynamic")]
    _library: windows::Win32::Foundation::HMODULE,
    
    // Function pointers
    open: SimConnect_Open,
    close: SimConnect_Close,
    #[allow(dead_code)]
    call_dispatch: SimConnect_CallDispatch,
    get_next_dispatch: SimConnect_GetNextDispatch,
    add_to_data_definition: SimConnect_AddToDataDefinition,
    request_data_on_sim_object: SimConnect_RequestDataOnSimObject,
    map_client_event_to_sim_event: SimConnect_MapClientEventToSimEvent,
    transmit_client_event: SimConnect_TransmitClientEvent,
    subscribe_to_system_event: SimConnect_SubscribeToSystemEvent,
}

// SAFETY: SimConnectApi function pointers are thread-safe
unsafe impl Send for SimConnectApi {}
unsafe impl Sync for SimConnectApi {}

impl SimConnectApi {
    /// Load SimConnect API (dynamic or static linking)
    pub fn new() -> Result<Self, SimConnectError> {
        #[cfg(feature = "dynamic")]
        {
            Self::load_dynamic()
        }
        #[cfg(feature = "static")]
        {
            Self::load_static()
        }
    }

    #[cfg(feature = "dynamic")]
    fn load_dynamic() -> Result<Self, SimConnectError> {
        use windows::Win32::System::LibraryLoader::LoadLibraryA;
        
        let library_name = CString::new("SimConnect.dll").unwrap();
        let library = unsafe { LoadLibraryA(PCSTR(library_name.as_ptr() as *const u8)) };
        
        let library = match library {
            Ok(lib) => {
                if lib.is_invalid() {
                    return Err(SimConnectError::LibraryNotFound);
                }
                lib
            }
            Err(_) => return Err(SimConnectError::LibraryNotFound),
        };

        // Load function pointers
        let open = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_Open"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_Open".to_string()));
            }
            std::mem::transmute::<_, SimConnect_Open>(proc.unwrap())
        };

        let close = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_Close"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_Close".to_string()));
            }
            std::mem::transmute::<_, SimConnect_Close>(proc.unwrap())
        };

        let call_dispatch = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_CallDispatch"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_CallDispatch".to_string()));
            }
            std::mem::transmute::<_, SimConnect_CallDispatch>(proc.unwrap())
        };

        let get_next_dispatch = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_GetNextDispatch"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_GetNextDispatch".to_string()));
            }
            std::mem::transmute::<_, SimConnect_GetNextDispatch>(proc.unwrap())
        };

        let add_to_data_definition = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_AddToDataDefinition"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_AddToDataDefinition".to_string()));
            }
            std::mem::transmute::<_, SimConnect_AddToDataDefinition>(proc.unwrap())
        };

        let request_data_on_sim_object = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_RequestDataOnSimObject"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_RequestDataOnSimObject".to_string()));
            }
            std::mem::transmute::<_, SimConnect_RequestDataOnSimObject>(proc.unwrap())
        };

        let map_client_event_to_sim_event = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_MapClientEventToSimEvent"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_MapClientEventToSimEvent".to_string()));
            }
            std::mem::transmute::<_, SimConnect_MapClientEventToSimEvent>(proc.unwrap())
        };

        let transmit_client_event = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_TransmitClientEvent"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_TransmitClientEvent".to_string()));
            }
            std::mem::transmute::<_, SimConnect_TransmitClientEvent>(proc.unwrap())
        };

        let subscribe_to_system_event = unsafe {
            let proc = GetProcAddress(library, s!("SimConnect_SubscribeToSystemEvent"));
            if proc.is_none() {
                return Err(SimConnectError::FunctionNotFound("SimConnect_SubscribeToSystemEvent".to_string()));
            }
            std::mem::transmute::<_, SimConnect_SubscribeToSystemEvent>(proc.unwrap())
        };

        Ok(Self {
            _library: library,
            open,
            close,
            call_dispatch,
            get_next_dispatch,
            add_to_data_definition,
            request_data_on_sim_object,
            map_client_event_to_sim_event,
            transmit_client_event,
            subscribe_to_system_event,
        })
    }

    #[cfg(feature = "static")]
    fn load_static() -> Result<Self, SimConnectError> {
        // For static linking, function pointers are resolved at link time
        extern "system" {
            fn SimConnect_Open(
                phSimConnect: *mut HSIMCONNECT,
                szName: PCSTR,
                hWnd: HWND,
                UserEventWin32: u32,
                hEventHandle: HANDLE,
                ConfigIndex: u32,
            ) -> HRESULT;
            fn SimConnect_Close(hSimConnect: HSIMCONNECT) -> HRESULT;
            fn SimConnect_CallDispatch(hSimConnect: HSIMCONNECT, pCallback: *mut c_void, pContext: *mut c_void) -> HRESULT;
            fn SimConnect_GetNextDispatch(
                hSimConnect: HSIMCONNECT,
                ppData: *mut *mut SIMCONNECT_RECV,
                pcbData: *mut u32,
            ) -> HRESULT;
            fn SimConnect_AddToDataDefinition(
                hSimConnect: HSIMCONNECT,
                DefineID: SIMCONNECT_DATADEFID,
                DatumName: PCSTR,
                UnitsName: PCSTR,
                DatumType: SIMCONNECT_DATATYPE,
                fEpsilon: f32,
                DatumID: u32,
            ) -> HRESULT;
            fn SimConnect_RequestDataOnSimObject(
                hSimConnect: HSIMCONNECT,
                RequestID: SIMCONNECT_REQUESTID,
                DefineID: SIMCONNECT_DATADEFID,
                ObjectID: SIMCONNECT_OBJECTID,
                Period: SIMCONNECT_PERIOD,
                Flags: u32,
                origin: u32,
                interval: u32,
                limit: u32,
            ) -> HRESULT;
            fn SimConnect_MapClientEventToSimEvent(
                hSimConnect: HSIMCONNECT,
                EventID: SIMCONNECT_EVENTID,
                EventName: PCSTR,
            ) -> HRESULT;
            fn SimConnect_TransmitClientEvent(
                hSimConnect: HSIMCONNECT,
                ObjectID: SIMCONNECT_OBJECTID,
                EventID: SIMCONNECT_EVENTID,
                dwData: u32,
                GroupID: SIMCONNECT_NOTIFICATIONGROUPID,
                Flags: u32,
            ) -> HRESULT;
            fn SimConnect_SubscribeToSystemEvent(
                hSimConnect: HSIMCONNECT,
                EventID: SIMCONNECT_EVENTID,
                SystemEventName: PCSTR,
            ) -> HRESULT;
        }

        Ok(Self {
            open: SimConnect_Open,
            close: SimConnect_Close,
            call_dispatch: SimConnect_CallDispatch,
            get_next_dispatch: SimConnect_GetNextDispatch,
            add_to_data_definition: SimConnect_AddToDataDefinition,
            request_data_on_sim_object: SimConnect_RequestDataOnSimObject,
            map_client_event_to_sim_event: SimConnect_MapClientEventToSimEvent,
            transmit_client_event: SimConnect_TransmitClientEvent,
            subscribe_to_system_event: SimConnect_SubscribeToSystemEvent,
        })
    }

    /// Open a connection to SimConnect
    pub fn open(
        &self,
        name: &str,
        hwnd: HWND,
        user_event: u32,
        event_handle: HANDLE,
        config_index: u32,
    ) -> Result<HSIMCONNECT, SimConnectError> {
        let name_cstr = CString::new(name).map_err(|_| SimConnectError::InvalidParameter)?;
        let mut handle: HSIMCONNECT = ptr::null_mut();

        let result = unsafe {
            (self.open)(
                &mut handle,
                PCSTR(name_cstr.as_ptr() as *const u8),
                hwnd,
                user_event,
                event_handle,
                config_index,
            )
        };

        if result.is_ok() {
            Ok(handle)
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Close a SimConnect connection
    pub fn close(&self, handle: HSIMCONNECT) -> Result<(), SimConnectError> {
        let result = unsafe { (self.close)(handle) };
        if result.is_ok() {
            Ok(())
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Get the next dispatch message
    pub fn get_next_dispatch(&self, handle: HSIMCONNECT) -> Result<Option<Vec<u8>>, SimConnectError> {
        let mut data_ptr: *mut SIMCONNECT_RECV = ptr::null_mut();
        let mut data_size: u32 = 0;

        let result = unsafe { (self.get_next_dispatch)(handle, &mut data_ptr, &mut data_size) };

        if result.is_ok() && !data_ptr.is_null() && data_size > 0 {
            let data = unsafe {
                std::slice::from_raw_parts(data_ptr as *const u8, data_size as usize).to_vec()
            };
            Ok(Some(data))
        } else if result.0 == 0x80004005u32 as i32 {
            // E_FAIL - no more messages
            Ok(None)
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Add a variable to a data definition
    pub fn add_to_data_definition(
        &self,
        handle: HSIMCONNECT,
        define_id: SIMCONNECT_DATADEFID,
        datum_name: &str,
        units_name: &str,
        datum_type: SIMCONNECT_DATATYPE,
        epsilon: f32,
        datum_id: u32,
    ) -> Result<(), SimConnectError> {
        let datum_cstr = CString::new(datum_name).map_err(|_| SimConnectError::InvalidParameter)?;
        let units_cstr = CString::new(units_name).map_err(|_| SimConnectError::InvalidParameter)?;

        let result = unsafe {
            (self.add_to_data_definition)(
                handle,
                define_id,
                PCSTR(datum_cstr.as_ptr() as *const u8),
                PCSTR(units_cstr.as_ptr() as *const u8),
                datum_type,
                epsilon,
                datum_id,
            )
        };

        if result.is_ok() {
            Ok(())
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Request data on a sim object
    pub fn request_data_on_sim_object(
        &self,
        handle: HSIMCONNECT,
        request_id: SIMCONNECT_REQUESTID,
        define_id: SIMCONNECT_DATADEFID,
        object_id: SIMCONNECT_OBJECTID,
        period: SIMCONNECT_PERIOD,
    ) -> Result<(), SimConnectError> {
        let result = unsafe {
            (self.request_data_on_sim_object)(handle, request_id, define_id, object_id, period, 0, 0, 0, 0)
        };

        if result.is_ok() {
            Ok(())
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Map a client event to a sim event
    pub fn map_client_event_to_sim_event(
        &self,
        handle: HSIMCONNECT,
        event_id: SIMCONNECT_EVENTID,
        event_name: &str,
    ) -> Result<(), SimConnectError> {
        let event_cstr = CString::new(event_name).map_err(|_| SimConnectError::InvalidParameter)?;

        let result = unsafe {
            (self.map_client_event_to_sim_event)(handle, event_id, PCSTR(event_cstr.as_ptr() as *const u8))
        };

        if result.is_ok() {
            Ok(())
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Transmit a client event
    pub fn transmit_client_event(
        &self,
        handle: HSIMCONNECT,
        object_id: SIMCONNECT_OBJECTID,
        event_id: SIMCONNECT_EVENTID,
        data: u32,
    ) -> Result<(), SimConnectError> {
        let result = unsafe { (self.transmit_client_event)(handle, object_id, event_id, data, 0, 0) };

        if result.is_ok() {
            Ok(())
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }

    /// Subscribe to a system event
    pub fn subscribe_to_system_event(
        &self,
        handle: HSIMCONNECT,
        event_id: SIMCONNECT_EVENTID,
        system_event_name: &str,
    ) -> Result<(), SimConnectError> {
        let event_cstr = CString::new(system_event_name).map_err(|_| SimConnectError::InvalidParameter)?;

        let result = unsafe {
            (self.subscribe_to_system_event)(handle, event_id, PCSTR(event_cstr.as_ptr() as *const u8))
        };

        if result.is_ok() {
            Ok(())
        } else {
            Err(SimConnectError::ApiError(result.0))
        }
    }
}

/// SimConnect error types
#[derive(Debug, thiserror::Error)]
pub enum SimConnectError {
    #[error("SimConnect library not found")]
    LibraryNotFound,
    #[error("Function not found: {0}")]
    FunctionNotFound(String),
    #[error("Invalid parameter")]
    InvalidParameter,
    #[error("SimConnect API error: 0x{0:08X}")]
    ApiError(i32),
}

/// Constants for common SimConnect values
pub mod constants {
    use super::*;

    /// User aircraft object ID
    pub const SIMCONNECT_OBJECT_ID_USER: SIMCONNECT_OBJECTID = 0;

    /// Data request flags
    pub const SIMCONNECT_DATA_REQUEST_FLAG_CHANGED: u32 = 0x00000001;
    pub const SIMCONNECT_DATA_REQUEST_FLAG_TAGGED: u32 = 0x00000002;

    /// Common data definition IDs
    pub const DATA_DEFINITION_AIRCRAFT: SIMCONNECT_DATADEFID = 1;
    pub const DATA_DEFINITION_KINEMATICS: SIMCONNECT_DATADEFID = 2;
    pub const DATA_DEFINITION_ENGINE: SIMCONNECT_DATADEFID = 3;
    pub const DATA_DEFINITION_ENVIRONMENT: SIMCONNECT_DATADEFID = 4;

    /// Common request IDs
    pub const REQUEST_AIRCRAFT_DATA: SIMCONNECT_REQUESTID = 1;
    pub const REQUEST_KINEMATICS_DATA: SIMCONNECT_REQUESTID = 2;
    pub const REQUEST_ENGINE_DATA: SIMCONNECT_REQUESTID = 3;
    pub const REQUEST_ENVIRONMENT_DATA: SIMCONNECT_REQUESTID = 4;

    /// Common event IDs
    pub const EVENT_AIRCRAFT_LOADED: SIMCONNECT_EVENTID = 1;
    pub const EVENT_SIM_START: SIMCONNECT_EVENTID = 2;
    pub const EVENT_SIM_STOP: SIMCONNECT_EVENTID = 3;
    pub const EVENT_PAUSE: SIMCONNECT_EVENTID = 4;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simconnect_api_creation() {
        // This test will only pass if SimConnect.dll is available
        match SimConnectApi::new() {
            Ok(_api) => {
                // API loaded successfully
            }
            Err(SimConnectError::LibraryNotFound) => {
                // Expected on systems without MSFS/SimConnect
                println!("SimConnect library not found - this is expected on systems without MSFS");
            }
            Err(e) => {
                panic!("Unexpected error loading SimConnect API: {}", e);
            }
        }
    }

    #[test]
    fn test_constants() {
        assert_eq!(constants::SIMCONNECT_OBJECT_ID_USER, 0);
        assert_eq!(constants::DATA_DEFINITION_AIRCRAFT, 1);
        assert_eq!(constants::REQUEST_AIRCRAFT_DATA, 1);
    }

    #[test]
    fn test_enums() {
        assert_eq!(SIMCONNECT_DATATYPE::FLOAT64 as u32, 4);
        assert_eq!(SIMCONNECT_PERIOD::SIM_FRAME as u32, 3);
        assert_eq!(SIMCONNECT_RECV_ID::SIMOBJECT_DATA as u32, 8);
    }
}