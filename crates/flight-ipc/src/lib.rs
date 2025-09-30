//! Flight Hub IPC Layer
//!
//! Provides protobuf-based IPC communication between Flight Hub components
//! using named pipes on Windows and Unix domain sockets on Linux.

pub mod proto {
    tonic::include_proto!("flight.v1");
}

pub use proto::*;
