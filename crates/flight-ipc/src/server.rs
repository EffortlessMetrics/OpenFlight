// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC server implementation
//!
//! Provides [`IpcServer`] for starting and gracefully shutting down the gRPC
//! server, plus the [`DeviceManager`] / [`ProfileManager`] traits consumed
//! by the handler layer.

use crate::{
    ServerConfig,
    handlers::{DefaultServiceContext, FlightServiceHandler, MockServiceContext, ServiceContext},
    proto::{
        ApplyProfileRequest, ApplyProfileResponse, Device, DeviceCapabilities, DeviceStatus,
        DeviceType, ListDevicesRequest, ListDevicesResponse,
        flight_service_server::FlightServiceServer as GrpcFlightServiceServer,
    },
};

use anyhow::Result;
use flight_core::watchdog::WatchdogSystem;
use flight_hid::{HidAdapter, device_support};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;
use tonic::Status;
use tracing::info;

// ---------------------------------------------------------------------------
// Domain manager traits (kept from original, used by DefaultServiceContext)
// ---------------------------------------------------------------------------

/// Device manager trait (to be implemented by actual device management)
pub trait DeviceManager: Send + Sync {
    /// Return the full list of connected devices matching the given filter
    fn list_devices(&self, request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status>;
}

/// Profile manager trait (to be implemented by actual profile management)
pub trait ProfileManager: Send + Sync {
    /// Apply the profile described in `request` and return the outcome
    fn apply_profile(&self, request: &ApplyProfileRequest) -> Result<ApplyProfileResponse, Status>;
}

// ---------------------------------------------------------------------------
// Mock implementations
// ---------------------------------------------------------------------------

/// Mock device manager for testing — always returns empty
#[derive(Debug)]
pub struct MockDeviceManager;

impl DeviceManager for MockDeviceManager {
    fn list_devices(&self, _request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status> {
        Ok(ListDevicesResponse {
            devices: vec![],
            total_count: 0,
        })
    }
}

/// Mock profile manager for testing — always succeeds
#[derive(Debug)]
pub struct MockProfileManager;

impl ProfileManager for MockProfileManager {
    fn apply_profile(
        &self,
        _request: &ApplyProfileRequest,
    ) -> Result<ApplyProfileResponse, Status> {
        Ok(ApplyProfileResponse {
            success: true,
            error_message: String::new(),
            validation_errors: vec![],
            effective_profile_hash: "mock-hash".to_string(),
            compile_time_ms: 10,
        })
    }
}

// ---------------------------------------------------------------------------
// HidDeviceManager — real HID-backed device enumeration
// ---------------------------------------------------------------------------

/// HID-backed device manager for basic device enumeration.
pub struct HidDeviceManager {
    adapter: Mutex<HidAdapter>,
}

impl std::fmt::Debug for HidDeviceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HidDeviceManager").finish()
    }
}

impl HidDeviceManager {
    /// Create a new manager backed by the system HID stack
    pub fn new() -> Self {
        let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
        let adapter = HidAdapter::new(watchdog);
        Self {
            adapter: Mutex::new(adapter),
        }
    }
}

impl Default for HidDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager for HidDeviceManager {
    fn list_devices(&self, request: &ListDevicesRequest) -> Result<ListDevicesResponse, Status> {
        let mut adapter = self
            .adapter
            .lock()
            .map_err(|_| Status::internal("HID adapter lock poisoned"))?;

        adapter
            .start()
            .map_err(|e| Status::internal(format!("HID adapter start failed: {}", e)))?;

        let device_infos = adapter.get_all_devices();
        let stecs_metadata_overlays = stecs_metadata_overlays(&device_infos);
        let gladiator_metadata_overlays = gladiator_metadata_overlays(&device_infos);

        let mut devices = Vec::new();
        for device_info in device_infos {
            let device_type = classify_device_type(device_info);
            if !request.filter_types.is_empty()
                && !request.filter_types.contains(&(device_type as i32))
            {
                continue;
            }

            let mut metadata = build_device_metadata(device_info);
            if let Some(stecs_overlay) = stecs_metadata_overlays.get(&device_info.device_path) {
                metadata.extend(stecs_overlay.clone());
            }
            if let Some(gladiator_overlay) =
                gladiator_metadata_overlays.get(&device_info.device_path)
            {
                metadata.extend(gladiator_overlay.clone());
            }

            let device = Device {
                id: device_info
                    .serial_number
                    .clone()
                    .unwrap_or_else(|| device_info.device_path.clone()),
                name: device_name(device_info),
                r#type: device_type.into(),
                status: DeviceStatus::Connected.into(),
                capabilities: Some(DeviceCapabilities {
                    supports_force_feedback: false,
                    supports_raw_torque: false,
                    max_torque_nm: 0,
                    min_period_us: 1000,
                    has_health_stream: false,
                    supported_protocols: vec!["hid".to_string()],
                }),
                health: None,
                metadata,
            };

            devices.push(device);
        }

        Ok(ListDevicesResponse {
            total_count: devices.len() as i32,
            devices,
        })
    }
}

// ---------------------------------------------------------------------------
// ServerHandle — returned by IpcServer::start
// ---------------------------------------------------------------------------

/// Handle to a running gRPC server. Call [`shutdown`](ServerHandle::shutdown)
/// for graceful termination.
pub struct ServerHandle {
    shutdown_tx: watch::Sender<bool>,
    join_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    addr: SocketAddr,
}

impl ServerHandle {
    /// The local address the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Signal the server to shut down and wait for it to finish.
    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown_tx.send(true);
        self.join_handle
            .await
            .map_err(|e| anyhow::anyhow!("Server task panicked: {e}"))?
            .map_err(|e| anyhow::anyhow!("Server error: {e}"))
    }
}

// ---------------------------------------------------------------------------
// IpcServer — the high-level gRPC server wrapper
// ---------------------------------------------------------------------------

/// High-level gRPC server for the Flight Hub IPC layer.
///
/// ```rust,no_run
/// use flight_ipc::server::IpcServer;
/// use flight_ipc::ServerConfig;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let server = IpcServer::new(ServerConfig::default());
///     let handle = server.start("127.0.0.1:0".parse()?).await?;
///     // … later …
///     handle.shutdown().await?;
///     Ok(())
/// }
/// ```
pub struct IpcServer<C: ServiceContext = DefaultServiceContext> {
    handler: FlightServiceHandler<C>,
    config: ServerConfig,
}

impl IpcServer<DefaultServiceContext> {
    /// Create a server backed by default production subsystems.
    pub fn new(config: ServerConfig) -> Self {
        let device_mgr: Arc<dyn DeviceManager> = Arc::new(HidDeviceManager::new());
        let profile_mgr: Arc<dyn ProfileManager> = Arc::new(MockProfileManager);
        let ctx = Arc::new(DefaultServiceContext::new(
            config.clone(),
            device_mgr,
            profile_mgr,
        ));
        let handler = FlightServiceHandler::new(ctx, config.clone());
        Self { handler, config }
    }
}

impl IpcServer<MockServiceContext> {
    /// Create a server backed by [`MockServiceContext`] — useful for tests.
    pub fn new_mock(config: ServerConfig) -> Self {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, config.clone());
        Self { handler, config }
    }
}

impl<C: ServiceContext> IpcServer<C> {
    /// Create a server with a custom [`ServiceContext`].
    pub fn with_context(ctx: Arc<C>, config: ServerConfig) -> Self {
        let handler = FlightServiceHandler::new(ctx, config.clone());
        Self { handler, config }
    }

    /// Start serving on `addr`. Returns a [`ServerHandle`] for graceful
    /// shutdown.
    pub async fn start(self, addr: SocketAddr) -> Result<ServerHandle> {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        info!("Flight IPC server listening on {}", local_addr);

        let svc = GrpcFlightServiceServer::new(self.handler);
        let max_conns = self.config.max_connections;
        let transport_config = crate::transport::TransportConfig {
            request_timeout: self.config.request_timeout,
            ..crate::transport::TransportConfig::default()
        };

        let join_handle = tokio::spawn(async move {
            let builder = tonic::transport::Server::builder();
            let builder = transport_config.configure_server(builder);
            builder
                .concurrency_limit_per_connection(max_conns)
                .add_service(svc)
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
        });

        Ok(ServerHandle {
            shutdown_tx,
            join_handle,
            addr: local_addr,
        })
    }
}

// ---------------------------------------------------------------------------
// Legacy FlightServer — thin wrapper kept for backward compatibility
// ---------------------------------------------------------------------------

/// Legacy server wrapper. Prefer [`IpcServer`] for new code.
pub struct FlightServer {
    config: ServerConfig,
}

impl FlightServer {
    /// Create a new server with default configuration
    pub fn new() -> Self {
        Self::with_config(ServerConfig::default())
    }

    /// Create a new server with custom configuration
    pub fn with_config(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Start the server (delegates to [`IpcServer`])
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = "127.0.0.1:50051".parse()?;
        let ipc = IpcServer::new(self.config);
        let handle = ipc.start(addr).await?;
        // Block until the server exits
        handle.shutdown().await?;
        Ok(())
    }
}

impl Default for FlightServer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Device classification helpers (internal, kept from original)
// ---------------------------------------------------------------------------

fn stecs_metadata_overlay(
    metadata: &device_support::VkbStecsInterfaceMetadata,
) -> HashMap<String, String> {
    let mut overlay = HashMap::new();
    overlay.insert(
        "stecs.physical_id".to_string(),
        metadata.physical_id.clone(),
    );
    overlay.insert(
        "stecs.virtual_controller_index".to_string(),
        metadata.virtual_controller_index.to_string(),
    );
    overlay.insert(
        "stecs.virtual_controller".to_string(),
        format!("VC{}", metadata.virtual_controller_index),
    );
    overlay.insert(
        "stecs.interface_count".to_string(),
        metadata.interface_count.to_string(),
    );
    overlay.insert(
        "stecs.multi_interface".to_string(),
        (metadata.interface_count > 1).to_string(),
    );
    let start_button = usize::from(metadata.virtual_controller_index) * 32 + 1;
    let end_button = start_button + 31;
    overlay.insert(
        "stecs.virtual_button_range".to_string(),
        format!("{}-{}", start_button, end_button),
    );
    overlay
}

fn stecs_metadata_overlays(
    devices: &[&flight_hid::HidDeviceInfo],
) -> HashMap<String, HashMap<String, String>> {
    let mut overlays = HashMap::new();
    for metadata in device_support::vkb_stecs_interface_metadata(devices.iter().copied()) {
        overlays.insert(
            metadata.device_path.clone(),
            stecs_metadata_overlay(&metadata),
        );
    }
    overlays
}

fn gladiator_metadata_overlay(
    metadata: &device_support::VkbGladiatorInterfaceMetadata,
) -> HashMap<String, String> {
    let mut overlay = HashMap::new();
    overlay.insert(
        "gladiator.physical_id".to_string(),
        metadata.physical_id.clone(),
    );
    overlay.insert(
        "gladiator.interface_index".to_string(),
        metadata.interface_index.to_string(),
    );
    overlay.insert(
        "gladiator.interface".to_string(),
        format!("IF{}", metadata.interface_index),
    );
    overlay.insert(
        "gladiator.interface_count".to_string(),
        metadata.interface_count.to_string(),
    );
    overlay.insert(
        "gladiator.multi_interface".to_string(),
        (metadata.interface_count > 1).to_string(),
    );
    overlay
}

fn gladiator_metadata_overlays(
    devices: &[&flight_hid::HidDeviceInfo],
) -> HashMap<String, HashMap<String, String>> {
    let mut overlays = HashMap::new();
    for metadata in device_support::vkb_gladiator_interface_metadata(devices.iter().copied()) {
        overlays.insert(
            metadata.device_path.clone(),
            gladiator_metadata_overlay(&metadata),
        );
    }
    overlays
}

fn classify_device_type(device_info: &flight_hid::HidDeviceInfo) -> DeviceType {
    if device_support::is_tflight_device(device_info) {
        return DeviceType::Joystick;
    }

    if device_support::is_vkb_stecs_device(device_info) {
        return DeviceType::Throttle;
    }

    if device_support::is_vkb_gladiator_device(device_info) {
        return DeviceType::Joystick;
    }

    if device_info.usage_page == device_support::USAGE_PAGE_GENERIC_DESKTOP
        && device_info.usage == device_support::USAGE_JOYSTICK
    {
        return DeviceType::Joystick;
    }

    DeviceType::Unspecified
}

fn device_name(device_info: &flight_hid::HidDeviceInfo) -> String {
    if let Some(model) = device_support::tflight_model(device_info) {
        return model.name().to_string();
    }

    if let Some(model) = device_support::vkb_stecs_variant(device_info) {
        return model.name().to_string();
    }

    if let Some(model) = device_support::vkb_gladiator_variant(device_info) {
        if let Some(product_name) = device_info.product_name.as_deref()
            && product_name.to_lowercase().contains("omni")
        {
            return product_name.to_string();
        }

        return model.name().to_string();
    }

    device_info
        .product_name
        .clone()
        .unwrap_or_else(|| "HID Device".to_string())
}

fn build_device_metadata(device_info: &flight_hid::HidDeviceInfo) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    metadata.insert(
        "vendor_id".to_string(),
        format!("{:04X}", device_info.vendor_id),
    );
    metadata.insert(
        "product_id".to_string(),
        format!("{:04X}", device_info.product_id),
    );
    metadata.insert("device_path".to_string(), device_info.device_path.clone());

    if let Some(manufacturer) = &device_info.manufacturer {
        metadata.insert("manufacturer".to_string(), manufacturer.clone());
    }

    if let Some(product_name) = &device_info.product_name {
        metadata.insert("product_name".to_string(), product_name.clone());
    }

    if let Some(discovery) = device_support::descriptor_discovery_from_device_info(device_info)
        && let Ok(json) = serde_json::to_string(&discovery)
    {
        metadata.insert("descriptor_discovery".to_string(), json);
    }

    if let Some(model) = device_support::tflight_model(device_info) {
        metadata.insert("device_family".to_string(), "tflight-hotas".to_string());
        metadata.insert("model".to_string(), model.name().to_string());
        metadata.insert(
            "is_legacy_pid".to_string(),
            device_support::is_hotas4_legacy_pid(device_info).to_string(),
        );

        let axis_mode = device_support::axis_mode_from_device_info(device_info);
        metadata.insert("axis_mode".to_string(), axis_mode.as_str().to_string());

        if let Some(warning) = device_support::axis_mode_warning(axis_mode) {
            metadata.insert("warning.axis_mode".to_string(), warning.to_string());
        }

        metadata.insert(
            "note.driver".to_string(),
            device_support::driver_note().to_string(),
        );
        metadata.insert(
            "note.pc_mode".to_string(),
            device_support::pc_mode_note(model).to_string(),
        );

        let mapping = device_support::tflight_default_mapping(axis_mode);
        metadata.insert("default_mapping".to_string(), mapping.as_hint_string());

        if let Some(note) = device_support::default_mapping_note(axis_mode) {
            metadata.insert("note.default_mapping".to_string(), note.to_string());
        }
    }

    if let Some(model) = device_support::vkb_stecs_variant(device_info) {
        metadata.insert("device_family".to_string(), "vkb-stecs".to_string());
        metadata.insert("model".to_string(), model.name().to_string());

        let control_map = device_support::vkb_stecs_control_map(model);
        if let Ok(json) = serde_json::to_string(control_map) {
            metadata.insert("control_map".to_string(), json);
        }
    }

    if let Some(model) = device_support::vkb_gladiator_variant(device_info) {
        metadata.insert(
            "device_family".to_string(),
            "vkb-gladiator-nxt-evo".to_string(),
        );
        metadata.insert("model".to_string(), model.name().to_string());

        let control_map = device_support::vkb_gladiator_control_map(model);
        if let Ok(json) = serde_json::to_string(control_map) {
            metadata.insert("control_map".to_string(), json);
        }

        if let Some(product_name) = device_info.product_name.as_deref()
            && product_name.to_lowercase().contains("omni")
        {
            metadata.insert("variant.omni".to_string(), "true".to_string());
        }
    }

    metadata
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::{FlightServiceHandler, MockServiceContext};
    use crate::proto::flight_service_server::FlightService as GrpcFlightService;
    use crate::proto::{
        GetServiceInfoRequest, NegotiateFeaturesRequest, ServiceStatus, TransportType,
    };
    use tonic::Request;

    #[tokio::test]
    async fn test_feature_negotiation() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let request = Request::new(NegotiateFeaturesRequest {
            client_version: "1.0.0".to_string(),
            supported_features: vec!["device-management".to_string()],
            preferred_transport: TransportType::NamedPipes.into(),
        });

        let response = handler.negotiate_features(request).await.unwrap();
        let response = response.into_inner();

        assert!(response.success);
        assert!(
            response
                .enabled_features
                .contains(&"device-management".to_string())
        );
    }

    #[tokio::test]
    async fn test_service_info() {
        let ctx = Arc::new(MockServiceContext::new());
        let handler = FlightServiceHandler::new(ctx, ServerConfig::default());

        let request = Request::new(GetServiceInfoRequest {});

        let response = handler.get_service_info(request).await.unwrap();
        let response = response.into_inner();

        assert_eq!(response.version, crate::PROTOCOL_VERSION);
        assert_eq!(response.status(), ServiceStatus::Running);
    }

    #[tokio::test]
    async fn test_ipc_server_start_and_shutdown() {
        let config = ServerConfig::default();
        let server = IpcServer::new_mock(config);
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let handle = server.start(addr).await.unwrap();
        assert_ne!(handle.addr().port(), 0);
        handle.shutdown().await.unwrap();
    }

    #[test]
    fn test_tflight_legacy_metadata_includes_guidance() {
        let device = flight_hid::HidDeviceInfo {
            vendor_id: device_support::THRUSTMASTER_VENDOR_ID,
            product_id: device_support::TFLIGHT_HOTAS_4_PID_LEGACY,
            serial_number: Some("legacy-1".to_string()),
            manufacturer: Some("Thrustmaster".to_string()),
            product_name: Some("T.Flight HOTAS 4".to_string()),
            device_path: "/dev/test-legacy".to_string(),
            usage_page: device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: device_support::USAGE_JOYSTICK,
            report_descriptor: None,
        };

        let metadata = build_device_metadata(&device);
        assert_eq!(
            metadata.get("device_family").map(String::as_str),
            Some("tflight-hotas")
        );
        assert_eq!(
            metadata.get("is_legacy_pid").map(String::as_str),
            Some("true")
        );
        assert!(
            metadata
                .get("note.pc_mode")
                .is_some_and(|note| note.contains("Share+Option+PS"))
        );
    }

    #[test]
    fn test_tflight_hotas_one_metadata_is_not_legacy() {
        let device = flight_hid::HidDeviceInfo {
            vendor_id: device_support::THRUSTMASTER_VENDOR_ID,
            product_id: device_support::TFLIGHT_HOTAS_ONE_PID,
            serial_number: Some("one-1".to_string()),
            manufacturer: Some("Thrustmaster".to_string()),
            product_name: Some("T.Flight HOTAS One".to_string()),
            device_path: "/dev/test-one".to_string(),
            usage_page: device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: device_support::USAGE_JOYSTICK,
            report_descriptor: None,
        };

        let metadata = build_device_metadata(&device);
        assert_eq!(
            metadata.get("is_legacy_pid").map(String::as_str),
            Some("false")
        );
        assert!(
            metadata
                .get("note.pc_mode")
                .is_some_and(|note| note.contains("Guide"))
        );
    }

    #[test]
    fn test_stecs_metadata_overlays_include_virtual_controller_details() {
        let vc0 = flight_hid::HidDeviceInfo {
            vendor_id: device_support::VKB_VENDOR_ID,
            product_id: device_support::VKB_STECS_RIGHT_SPACE_STANDARD_PID,
            serial_number: Some("STECS-SERIAL".to_string()),
            manufacturer: Some("VKB".to_string()),
            product_name: Some("VKB STECS".to_string()),
            device_path: r"\\?\hid#vid_231d&pid_013c&mi_00#7".to_string(),
            usage_page: device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: device_support::USAGE_JOYSTICK,
            report_descriptor: None,
        };

        let mut vc1 = vc0.clone();
        vc1.device_path = r"\\?\hid#vid_231d&pid_013c&mi_01#7".to_string();

        let overlays = stecs_metadata_overlays(&[&vc0, &vc1]);
        assert_eq!(overlays.len(), 2);

        let first = overlays.get(&vc0.device_path).expect("vc0 overlay");
        let second = overlays.get(&vc1.device_path).expect("vc1 overlay");

        assert_eq!(
            first
                .get("stecs.virtual_controller_index")
                .map(String::as_str),
            Some("0")
        );
        assert_eq!(
            second
                .get("stecs.virtual_controller_index")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            first.get("stecs.virtual_button_range").map(String::as_str),
            Some("1-32")
        );
        assert_eq!(
            second.get("stecs.virtual_button_range").map(String::as_str),
            Some("33-64")
        );
        assert_eq!(
            first.get("stecs.interface_count").map(String::as_str),
            Some("2")
        );
    }

    #[test]
    fn test_gladiator_metadata_includes_control_map() {
        let device = flight_hid::HidDeviceInfo {
            vendor_id: device_support::VKB_VENDOR_ID,
            product_id: device_support::VKB_GLADIATOR_NXT_EVO_RIGHT_PID,
            serial_number: Some("SCG-RIGHT-1".to_string()),
            manufacturer: Some("VKB".to_string()),
            product_name: Some("VKB Gladiator NXT EVO Right".to_string()),
            device_path: "/dev/test-gladiator-right".to_string(),
            usage_page: device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: device_support::USAGE_JOYSTICK,
            report_descriptor: None,
        };

        let metadata = build_device_metadata(&device);
        assert_eq!(
            metadata.get("device_family").map(String::as_str),
            Some("vkb-gladiator-nxt-evo")
        );

        let control_map = metadata
            .get("control_map")
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .expect("control_map metadata should be valid JSON");

        assert_eq!(
            control_map
                .get("schema")
                .and_then(serde_json::Value::as_str),
            Some("flight.device-map/1")
        );
        assert_eq!(
            control_map
                .get("axes")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(8)
        );
    }

    #[test]
    fn test_gladiator_metadata_overlays_include_interface_details() {
        let if0 = flight_hid::HidDeviceInfo {
            vendor_id: device_support::VKB_VENDOR_ID,
            product_id: device_support::VKB_GLADIATOR_NXT_EVO_LEFT_PID,
            serial_number: Some("SCG-SERIAL".to_string()),
            manufacturer: Some("VKB".to_string()),
            product_name: Some("VKB Gladiator NXT EVO Left".to_string()),
            device_path: r"\\?\hid#vid_231d&pid_0201&mi_00#7".to_string(),
            usage_page: device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: device_support::USAGE_JOYSTICK,
            report_descriptor: None,
        };

        let mut if1 = if0.clone();
        if1.device_path = r"\\?\hid#vid_231d&pid_0201&mi_01#7".to_string();

        let overlays = gladiator_metadata_overlays(&[&if0, &if1]);
        assert_eq!(overlays.len(), 2);

        let first = overlays.get(&if0.device_path).expect("if0 overlay");
        let second = overlays.get(&if1.device_path).expect("if1 overlay");

        assert_eq!(
            first.get("gladiator.interface_index").map(String::as_str),
            Some("0")
        );
        assert_eq!(
            second.get("gladiator.interface_index").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            first.get("gladiator.interface_count").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            first.get("gladiator.multi_interface").map(String::as_str),
            Some("true")
        );
    }
}
