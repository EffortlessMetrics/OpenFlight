// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! WebSocket live-update endpoint for the Flight Hub dashboard.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::sync::broadcast;

use crate::api::ApiState;
use crate::dashboard::WsMessage;

/// Broadcast channel wrapper for WebSocket messages.
pub struct WsBroadcast {
    tx: broadcast::Sender<WsMessage>,
}

impl WsBroadcast {
    /// Create a new broadcast with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Send a message to all connected WebSocket clients.
    pub fn send(&self, msg: WsMessage) -> Result<usize, broadcast::error::SendError<WsMessage>> {
        self.tx.send(msg)
    }

    /// Subscribe to the broadcast.
    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }

    /// Get the current number of active receivers.
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// Default publish rate for live axis updates.
pub const DEFAULT_PUBLISH_RATE: Duration = Duration::from_millis(50); // 20 Hz

/// Build a router with the WebSocket endpoint.
pub fn ws_router(state: ApiState) -> Router {
    Router::new()
        .route("/ws/live", get(ws_upgrade_handler))
        .with_state(state)
}

async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state.broadcast))
}

async fn handle_ws(mut socket: WebSocket, broadcast: Arc<WsBroadcast>) {
    let mut rx = broadcast.subscribe();
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        let text = match serde_json::to_string(&msg) {
                            Ok(t) => t,
                            Err(_) => continue,
                        };
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_broadcast_send_without_receivers() {
        let bc = WsBroadcast::new(16);
        // No subscribers yet, send should return Err (no receivers)
        let result = bc.send(WsMessage::AxisUpdate {
            axis: "roll".into(),
            value: 0.5,
        });
        assert!(result.is_err());
    }

    #[test]
    fn ws_broadcast_send_with_receiver() {
        let bc = WsBroadcast::new(16);
        let mut rx = bc.subscribe();
        let result = bc.send(WsMessage::AxisUpdate {
            axis: "pitch".into(),
            value: -0.3,
        });
        assert!(result.is_ok());
        let msg = rx.try_recv().unwrap();
        match msg {
            WsMessage::AxisUpdate { axis, value } => {
                assert_eq!(axis, "pitch");
                assert!((value - (-0.3)).abs() < f64::EPSILON);
            }
            _ => panic!("unexpected message variant"),
        }
    }

    #[test]
    fn ws_broadcast_receiver_count() {
        let bc = WsBroadcast::new(16);
        assert_eq!(bc.receiver_count(), 0);
        let _rx1 = bc.subscribe();
        assert_eq!(bc.receiver_count(), 1);
        let _rx2 = bc.subscribe();
        assert_eq!(bc.receiver_count(), 2);
        drop(_rx1);
        assert_eq!(bc.receiver_count(), 1);
    }

    #[test]
    fn ws_broadcast_multiple_messages() {
        let bc = WsBroadcast::new(16);
        let mut rx = bc.subscribe();

        bc.send(WsMessage::AxisUpdate {
            axis: "roll".into(),
            value: 0.1,
        })
        .unwrap();
        bc.send(WsMessage::DeviceEvent {
            device_id: "d1".into(),
            event: crate::dashboard::DeviceEventKind::Connected,
        })
        .unwrap();

        let m1 = rx.try_recv().unwrap();
        let m2 = rx.try_recv().unwrap();
        assert!(matches!(m1, WsMessage::AxisUpdate { .. }));
        assert!(matches!(m2, WsMessage::DeviceEvent { .. }));
    }

    #[test]
    fn ws_message_json_format() {
        let msg = WsMessage::AxisUpdate {
            axis: "yaw".into(),
            value: 0.0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"axis_update\""));
        assert!(json.contains("\"axis\":\"yaw\""));
    }

    #[test]
    fn ws_message_device_disconnect_json() {
        let msg = WsMessage::DeviceEvent {
            device_id: "hid-42".into(),
            event: crate::dashboard::DeviceEventKind::Disconnected,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"device_event\""));
        assert!(json.contains("\"event\":\"disconnected\""));
    }

    #[test]
    fn ws_message_adapter_event_json() {
        let msg = WsMessage::AdapterEvent {
            adapter: "xplane".into(),
            connected: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "adapter_event");
        assert_eq!(parsed["adapter"], "xplane");
        assert_eq!(parsed["connected"], true);
    }

    #[test]
    fn default_publish_rate_is_20hz() {
        assert_eq!(DEFAULT_PUBLISH_RATE, Duration::from_millis(50));
    }
}
