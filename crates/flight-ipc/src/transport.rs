// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Transport layer abstractions for cross-platform IPC

use anyhow::Result;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use flight_core::{SecurityManager, IpcClientInfo};
use std::path::PathBuf;
// Transport abstractions - actual tonic transport integration would go here

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Transport not supported on this platform")]
    UnsupportedPlatform,
    
    #[error("Connection timeout")]
    Timeout,
    
    #[error("Invalid address: {address}")]
    InvalidAddress { address: String },
}

/// Cross-platform transport abstraction
pub trait Transport: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static {}

/// Named pipe transport (Windows)
#[cfg(all(windows, feature = "named-pipes"))]
pub mod named_pipes {
    use super::*;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ClientOptions};
    
    pub struct NamedPipeTransport {
        inner: NamedPipeInner,
    }
    
    enum NamedPipeInner {
        Server(NamedPipeServer),
        Client(tokio::net::windows::named_pipe::NamedPipeClient),
    }
    
    impl NamedPipeTransport {
        pub async fn connect(address: &str) -> Result<Self, TransportError> {
            let client = ClientOptions::new().open(address)?;
            Ok(Self {
                inner: NamedPipeInner::Client(client),
            })
        }
        
        pub async fn bind(address: &str) -> Result<Self, TransportError> {
            let server = tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .create(address)?;
            
            Ok(Self {
                inner: NamedPipeInner::Server(server),
            })
        }
    }
    
    impl AsyncRead for NamedPipeTransport {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_read(cx, buf),
                NamedPipeInner::Client(client) => Pin::new(client).poll_read(cx, buf),
            }
        }
    }
    
    impl AsyncWrite for NamedPipeTransport {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_write(cx, buf),
                NamedPipeInner::Client(client) => Pin::new(client).poll_write(cx, buf),
            }
        }
        
        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_flush(cx),
                NamedPipeInner::Client(client) => Pin::new(client).poll_flush(cx),
            }
        }
        
        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
            match &mut self.inner {
                NamedPipeInner::Server(server) => Pin::new(server).poll_shutdown(cx),
                NamedPipeInner::Client(client) => Pin::new(client).poll_shutdown(cx),
            }
        }
    }
    
    impl Transport for NamedPipeTransport {}
}

/// Unix domain socket transport (Linux/macOS)
#[cfg(all(unix, feature = "unix-sockets"))]
pub mod unix_sockets {
    use super::*;
    use tokio::net::{UnixListener, UnixStream};
    
    pub struct UnixSocketTransport {
        stream: UnixStream,
    }
    
    impl UnixSocketTransport {
        pub async fn connect(address: &str) -> Result<Self, TransportError> {
            let stream = UnixStream::connect(address).await?;
            Ok(Self { stream })
        }
        
        pub async fn bind(address: &str) -> Result<UnixListener, TransportError> {
            // Remove existing socket file if it exists
            let _ = std::fs::remove_file(address);
            let listener = UnixListener::bind(address)?;
            Ok(listener)
        }
        
        pub fn from_stream(stream: UnixStream) -> Self {
            Self { stream }
        }
    }
    
    impl AsyncRead for UnixSocketTransport {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }
    
    impl AsyncWrite for UnixSocketTransport {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            Pin::new(&mut self.stream).poll_write(cx, buf)
        }
        
        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
            Pin::new(&mut self.stream).poll_flush(cx)
        }
        
        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
            Pin::new(&mut self.stream).poll_shutdown(cx)
        }
    }
    
    impl Transport for UnixSocketTransport {}
}

/// Create a transport based on the specified type and address with ACL validation
pub async fn create_transport_with_acl(
    transport_type: crate::TransportType,
    address: &str,
    is_server: bool,
    security_manager: Option<&SecurityManager>,
) -> Result<Box<dyn Transport>, TransportError> {
    // Validate ACL if security manager is provided and this is a client connection
    if let Some(security_manager) = security_manager {
        if !is_server {
            let client_info = get_client_info()?;
            security_manager.validate_ipc_acl(&client_info)
                .map_err(|e| TransportError::InvalidAddress { 
                    address: format!("ACL validation failed: {}", e) 
                })?;
        }
    }
    
    create_transport(transport_type, address, is_server).await
}

/// Create a transport based on the specified type and address
pub async fn create_transport(
    transport_type: crate::TransportType,
    address: &str,
    is_server: bool,
) -> Result<Box<dyn Transport>, TransportError> {
    match transport_type {
        #[cfg(all(windows, feature = "named-pipes"))]
        crate::TransportType::NamedPipes => {
            if is_server {
                let transport = named_pipes::NamedPipeTransport::bind(address).await?;
                Ok(Box::new(transport))
            } else {
                let transport = named_pipes::NamedPipeTransport::connect(address).await?;
                Ok(Box::new(transport))
            }
        }
        
        #[cfg(all(unix, feature = "unix-sockets"))]
        crate::TransportType::UnixSockets => {
            if is_server {
                return Err(TransportError::UnsupportedPlatform);
            } else {
                let transport = unix_sockets::UnixSocketTransport::connect(address).await?;
                Ok(Box::new(transport))
            }
        }
        
        _ => Err(TransportError::UnsupportedPlatform),
    }
}

/// Get client information for ACL validation
fn get_client_info() -> Result<IpcClientInfo, TransportError> {
    #[cfg(windows)]
    {
        // On Windows, get current process info
        let process_id = std::process::id();
        let user_id = get_current_user_sid()
            .unwrap_or_else(|_| "UNKNOWN".to_string());
        let executable_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("unknown"));
        
        Ok(IpcClientInfo {
            user_id,
            process_id,
            executable_path,
        })
    }
    
    #[cfg(unix)]
    {
        // On Unix, get current process info
        let process_id = std::process::id();
        let user_id = unsafe { libc::getuid() }.to_string();
        let executable_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("unknown"));
        
        Ok(IpcClientInfo {
            user_id,
            process_id,
            executable_path,
        })
    }
}

#[cfg(windows)]
fn get_current_user_sid() -> Result<String, std::io::Error> {
    // In a real implementation, this would use Windows APIs to get the current user SID
    // For now, return a placeholder
    Ok("S-1-5-21-000000000-000000000-000000000-1000".to_string())
}