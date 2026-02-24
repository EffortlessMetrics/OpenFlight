// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VR overlay service — drives the notification queue, state updates, and renderer.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, watch};

use crate::{
    OverlayConfig, OverlayError, OverlayState,
    notification::{NotificationQueue, OverlayNotification, Severity},
    renderer::OverlayRenderer,
};

/// Commands that can be sent to the overlay service at runtime.
#[derive(Debug)]
pub enum OverlayCommand {
    /// Show the overlay panel.
    Show,
    /// Hide the overlay panel.
    Hide,
    /// Toggle visibility.
    Toggle,
    /// Push a notification.
    Notify {
        message: String,
        severity: Severity,
        ttl_secs: Option<u64>,
    },
    /// Update the active profile name shown in the header.
    SetProfile(String),
    /// Update the overlay state directly (e.g. from a bus callback).
    UpdateState(Box<OverlayState>),
    /// Gracefully shut down the service.
    Shutdown,
}

/// Handle returned to callers when the overlay service is spawned.
///
/// Used to send commands and query current status.
#[derive(Clone)]
pub struct OverlayHandle {
    tx: tokio::sync::mpsc::Sender<OverlayCommand>,
    state_rx: watch::Receiver<OverlayState>,
}

impl OverlayHandle {
    /// Send a command to the overlay service (non-blocking).
    pub async fn send(&self, cmd: OverlayCommand) -> Result<(), OverlayError> {
        self.tx.send(cmd).await.map_err(|_| OverlayError::ServiceShutdown)
    }

    /// Read the latest overlay state snapshot.
    pub fn state(&self) -> OverlayState {
        self.state_rx.borrow().clone()
    }

    /// Push a short notification.
    pub async fn notify(
        &self,
        message: impl Into<String>,
        severity: Severity,
        ttl_secs: u64,
    ) -> Result<(), OverlayError> {
        self.send(OverlayCommand::Notify {
            message: message.into(),
            severity,
            ttl_secs: Some(ttl_secs),
        })
        .await
    }

    /// Update the profile name shown in the overlay header.
    pub async fn set_profile(&self, name: impl Into<String>) -> Result<(), OverlayError> {
        self.send(OverlayCommand::SetProfile(name.into())).await
    }

    /// Toggle overlay visibility.
    pub async fn toggle(&self) -> Result<(), OverlayError> {
        self.send(OverlayCommand::Toggle).await
    }

    /// Shut down the overlay service.
    pub async fn shutdown(&self) -> Result<(), OverlayError> {
        self.send(OverlayCommand::Shutdown).await
    }
}

/// Main overlay service.
///
/// Drives a [`OverlayRenderer`] at `tick_hz` Hz, processes commands from
/// [`OverlayHandle`], and prunes expired notifications automatically.
pub struct OverlayService<R: OverlayRenderer> {
    config: OverlayConfig,
    renderer: Arc<Mutex<R>>,
    queue: NotificationQueue,
    state: OverlayState,
    cmd_rx: tokio::sync::mpsc::Receiver<OverlayCommand>,
    state_tx: watch::Sender<OverlayState>,
}

impl<R: OverlayRenderer> OverlayService<R> {
    /// Spawn the overlay service on the current tokio runtime.
    ///
    /// Returns an [`OverlayHandle`] for sending commands and reading state.
    pub fn spawn(config: OverlayConfig, renderer: R) -> OverlayHandle {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(64);
        let (state_tx, state_rx) = watch::channel(OverlayState::default());

        let queue = NotificationQueue::new(config.max_notifications);
        let renderer = Arc::new(Mutex::new(renderer));

        let svc = Self {
            config,
            renderer,
            queue,
            state: OverlayState::default(),
            cmd_rx,
            state_tx,
        };

        tokio::spawn(svc.run());

        OverlayHandle { tx: cmd_tx, state_rx }
    }

    async fn run(mut self) {
        let tick = Duration::from_millis(16); // ~60 Hz render loop
        let mut interval = tokio::time::interval(tick);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.queue.prune_expired();
                    // Update visibility in state
                    let _ = self.state_tx.send(self.state.clone());
                    if self.state.visible {
                        let mut r = self.renderer.lock().await;
                        let _ = r.render_frame(&self.state);
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        None | Some(OverlayCommand::Shutdown) => break,
                        Some(c) => self.handle_command(c).await,
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: OverlayCommand) {
        match cmd {
            OverlayCommand::Show => {
                self.state.visible = true;
                let mut r = self.renderer.lock().await;
                let _ = r.show();
            }
            OverlayCommand::Hide => {
                self.state.visible = false;
                let mut r = self.renderer.lock().await;
                let _ = r.hide();
            }
            OverlayCommand::Toggle => {
                self.state.toggle_visible();
                let mut r = self.renderer.lock().await;
                if self.state.visible {
                    let _ = r.show();
                } else {
                    let _ = r.hide();
                }
            }
            OverlayCommand::Notify { message, severity, ttl_secs } => {
                let ttl = ttl_secs
                    .unwrap_or(self.config.notification_ttl_secs)
                    .max(1);
                let n = OverlayNotification::new(message, severity, Duration::from_secs(ttl));
                self.queue.push(n);
            }
            OverlayCommand::SetProfile(name) => {
                self.state.profile_name = name;
            }
            OverlayCommand::UpdateState(new_state) => {
                self.state = *new_state;
            }
            OverlayCommand::Shutdown => unreachable!("handled in run loop"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::NullRenderer;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_spawn_and_shutdown() {
        let handle = OverlayService::spawn(OverlayConfig::minimal(), NullRenderer::new());
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_set_profile_reflected_in_state() {
        let handle = OverlayService::spawn(OverlayConfig::minimal(), NullRenderer::new());
        handle.set_profile("MSFS-747").await.unwrap();
        sleep(Duration::from_millis(50)).await;
        let s = handle.state();
        assert_eq!(s.profile_name, "MSFS-747");
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_toggle_changes_visibility() {
        let handle = OverlayService::spawn(OverlayConfig::minimal(), NullRenderer::new());
        let initial = handle.state().visible;
        handle.toggle().await.unwrap();
        sleep(Duration::from_millis(50)).await;
        let after = handle.state().visible;
        assert_ne!(initial, after);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_notify_sends_command() {
        let handle = OverlayService::spawn(OverlayConfig::minimal(), NullRenderer::new());
        handle.notify("Test notification", Severity::Info, 2).await.unwrap();
        sleep(Duration::from_millis(50)).await;
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_shutdown_handle_error_after_stop() {
        let handle = OverlayService::spawn(OverlayConfig::minimal(), NullRenderer::new());
        handle.shutdown().await.unwrap();
        sleep(Duration::from_millis(20)).await;
        // Second shutdown attempt should fail gracefully (service already gone)
        let result = handle.shutdown().await;
        assert!(result.is_err());
    }
}
