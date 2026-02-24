//! kRPC TCP connection: length-delimited protobuf framing over a TCP socket.

use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::KspError;
use crate::protocol::{
    Argument, ConnectionRequest, ConnectionResponse, ProcedureCall, Request, Response,
    connection_request, connection_response,
};

/// A connected kRPC RPC channel.
pub struct KrpcConnection {
    stream: TcpStream,
}

impl KrpcConnection {
    /// Establish a connection to the kRPC server at `host:port` using client
    /// `name` (visible in the kRPC server window inside KSP).
    pub async fn connect(host: &str, port: u16, name: &str) -> Result<Self, KspError> {
        let mut stream = TcpStream::connect(format!("{host}:{port}")).await?;

        let req = ConnectionRequest {
            r#type: connection_request::Type::Rpc as i32,
            name: name.to_string(),
            client_identifier: vec![],
        };
        Self::write_message_raw(&mut stream, &req).await?;

        let resp: ConnectionResponse = Self::read_message_raw(&mut stream).await?;
        if resp.status != connection_response::Status::Ok as i32 {
            return Err(KspError::ConnectionRejected(resp.message));
        }

        Ok(Self { stream })
    }

    /// Make a single procedure call and return the raw result bytes.
    pub async fn call(
        &mut self,
        service: &str,
        procedure: &str,
        args: Vec<Argument>,
    ) -> Result<Vec<u8>, KspError> {
        let results = self.call_batch(vec![(service, procedure, args)]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| KspError::Protocol("Empty batch response".to_string()))
    }

    /// Make several procedure calls in one round-trip.
    /// Returns one result `Vec<u8>` per call in the same order.
    pub async fn call_batch(
        &mut self,
        calls: Vec<(&str, &str, Vec<Argument>)>,
    ) -> Result<Vec<Vec<u8>>, KspError> {
        let req = Request {
            calls: calls
                .into_iter()
                .map(|(svc, proc, args)| ProcedureCall {
                    service: svc.to_string(),
                    procedure: proc.to_string(),
                    service_id: 0,
                    procedure_id: 0,
                    arguments: args,
                })
                .collect(),
        };
        Self::write_message_raw(&mut self.stream, &req).await?;
        let resp: Response = Self::read_message_raw(&mut self.stream).await?;

        let mut out = Vec::with_capacity(resp.results.len());
        for result in resp.results {
            if let Some(err) = result.error {
                return Err(KspError::ProcedureError {
                    service: err.service,
                    name: err.name,
                    description: err.description,
                });
            }
            out.push(result.value);
        }
        Ok(out)
    }

    // ── Internal I/O helpers ──────────────────────────────────────────────────

    async fn write_message_raw<M: Message>(
        stream: &mut TcpStream,
        msg: &M,
    ) -> Result<(), KspError> {
        let bytes = msg.encode_to_vec();
        // Write length as protobuf varint
        let mut len_buf = Vec::new();
        prost::encode_length_delimiter(bytes.len(), &mut len_buf).map_err(KspError::Encode)?;
        stream.write_all(&len_buf).await?;
        stream.write_all(&bytes).await?;
        Ok(())
    }

    async fn read_message_raw<M: Message + Default>(stream: &mut TcpStream) -> Result<M, KspError> {
        let len = read_varint(stream).await?;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;
        M::decode(buf.as_slice()).map_err(KspError::Decode)
    }
}

/// Read a single protobuf varint from an async reader.
async fn read_varint(stream: &mut TcpStream) -> Result<usize, KspError> {
    let mut value = 0usize;
    let mut shift = 0usize;
    loop {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).await?;
        let b = byte[0] as usize;
        value |= (b & 0x7F) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err(KspError::Protocol("Varint overflow".to_string()));
        }
    }
    Ok(value)
}
