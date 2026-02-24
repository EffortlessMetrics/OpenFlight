//! kRPC wire-protocol message definitions.
//!
//! kRPC uses length-delimited protobuf messages over a plain TCP socket.
//! These message types match the kRPC 0.5.x KRPC.proto schema.

use prost::Message;

// ── Connection messages ───────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Message)]
pub struct ConnectionRequest {
    /// 0 = RPC, 1 = STREAM
    #[prost(enumeration = "connection_request::Type", tag = "1")]
    pub r#type: i32,
    /// Human-readable client name shown in the kRPC server UI
    #[prost(string, tag = "2")]
    pub name: String,
    /// Client identifier bytes (empty for new connections)
    #[prost(bytes = "vec", tag = "3")]
    pub client_identifier: Vec<u8>,
}

pub mod connection_request {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, prost::Enumeration)]
    pub enum Type {
        Rpc = 0,
        Stream = 1,
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ConnectionResponse {
    #[prost(enumeration = "connection_response::Status", tag = "1")]
    pub status: i32,
    #[prost(string, tag = "2")]
    pub message: String,
    #[prost(bytes = "vec", tag = "3")]
    pub client_identifier: Vec<u8>,
}

pub mod connection_response {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, prost::Enumeration)]
    pub enum Status {
        Ok = 0,
        MalformedMessage = 1,
        Timeout = 2,
        WrongType = 3,
    }
}

// ── RPC request / response ────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Message)]
pub struct Request {
    #[prost(message, repeated, tag = "1")]
    pub calls: Vec<ProcedureCall>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProcedureCall {
    /// Service name, e.g. "SpaceCenter"
    #[prost(string, tag = "1")]
    pub service: String,
    /// Procedure name, e.g. "get_ActiveVessel"
    #[prost(string, tag = "2")]
    pub procedure: String,
    /// Optional numeric service ID (0 = use string name)
    #[prost(uint32, tag = "3")]
    pub service_id: u32,
    /// Optional numeric procedure ID (0 = use string name)
    #[prost(uint32, tag = "4")]
    pub procedure_id: u32,
    #[prost(message, repeated, tag = "5")]
    pub arguments: Vec<Argument>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Argument {
    /// Zero-based argument position
    #[prost(uint32, tag = "1")]
    pub position: u32,
    /// Argument value encoded as a protobuf message
    #[prost(bytes = "vec", tag = "2")]
    pub value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Response {
    #[prost(double, tag = "1")]
    pub time: f64,
    #[prost(message, repeated, tag = "2")]
    pub results: Vec<ProcedureResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProcedureResult {
    #[prost(message, optional, tag = "1")]
    pub error: Option<KrpcError>,
    /// Return value encoded as a protobuf message
    #[prost(bytes = "vec", tag = "2")]
    pub value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct KrpcError {
    #[prost(string, tag = "1")]
    pub service: String,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub description: String,
    #[prost(string, tag = "4")]
    pub stack_trace: String,
}

// ── Value encoding helpers ────────────────────────────────────────────────────

/// Encode a `u64` remote-object handle as a procedure argument value.
pub fn encode_object(handle: u64) -> Vec<u8> {
    #[derive(Message)]
    struct Wrap {
        #[prost(uint64, tag = "1")]
        v: u64,
    }
    Wrap { v: handle }.encode_to_vec()
}

/// Decode a `u64` remote-object handle from procedure result bytes.
pub fn decode_object(bytes: &[u8]) -> Result<u64, prost::DecodeError> {
    #[derive(Message)]
    struct Wrap {
        #[prost(uint64, tag = "1")]
        v: u64,
    }
    Ok(Wrap::decode(bytes)?.v)
}

/// Decode a `f64` scalar from procedure result bytes.
pub fn decode_double(bytes: &[u8]) -> Result<f64, prost::DecodeError> {
    #[derive(Message)]
    struct Wrap {
        #[prost(double, tag = "1")]
        v: f64,
    }
    Ok(Wrap::decode(bytes)?.v)
}

/// Decode a `f32` scalar from procedure result bytes.
pub fn decode_float(bytes: &[u8]) -> Result<f32, prost::DecodeError> {
    #[derive(Message)]
    struct Wrap {
        #[prost(float, tag = "1")]
        v: f32,
    }
    Ok(Wrap::decode(bytes)?.v)
}

/// Decode a `bool` from procedure result bytes.
pub fn decode_bool(bytes: &[u8]) -> Result<bool, prost::DecodeError> {
    #[derive(Message)]
    struct Wrap {
        #[prost(bool, tag = "1")]
        v: bool,
    }
    Ok(Wrap::decode(bytes)?.v)
}

/// Decode a `string` from procedure result bytes.
pub fn decode_string(bytes: &[u8]) -> Result<String, prost::DecodeError> {
    #[derive(Message)]
    struct Wrap {
        #[prost(string, tag = "1")]
        v: String,
    }
    Ok(Wrap::decode(bytes)?.v)
}

/// Decode an `i32` enum value from procedure result bytes.
pub fn decode_int32(bytes: &[u8]) -> Result<i32, prost::DecodeError> {
    #[derive(Message)]
    struct Wrap {
        #[prost(int32, tag = "1")]
        v: i32,
    }
    Ok(Wrap::decode(bytes)?.v)
}
