use thiserror::Error;

#[derive(Debug, Error)]
pub enum KspError {
    #[error("TCP connection error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("Protobuf encode error: {0}")]
    Encode(#[from] prost::EncodeError),

    #[error("kRPC connection rejected: {0}")]
    ConnectionRejected(String),

    #[error("kRPC procedure error – {service}::{name}: {description}")]
    ProcedureError {
        service: String,
        name: String,
        description: String,
    },

    #[error("kRPC protocol error: {0}")]
    Protocol(String),

    #[error("No active vessel")]
    NoActiveVessel,

    #[error("Adapter is not connected")]
    NotConnected,
}
