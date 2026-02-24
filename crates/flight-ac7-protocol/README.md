# flight-ac7-protocol

`flight-ac7-protocol` defines the AC7 telemetry payload format used by Flight Hub.

## Responsibilities

- Define a stable telemetry JSON schema for AC7 bridge plugins.
- Validate field ranges before adapter conversion.
- Keep protocol parsing separate from transport and bus publishing.

## Example

```rust
use flight_ac7_protocol::Ac7TelemetryPacket;

let packet = Ac7TelemetryPacket::from_json_str(
    r#"{
      "schema":"flight.ac7.telemetry/1",
      "timestamp_ms": 1000,
      "aircraft":"F-16C",
      "state":{"altitude_m":1200.0,"speed_mps":210.0},
      "controls":{"pitch":0.1,"roll":-0.2,"yaw":0.0,"throttle":0.7}
    }"#,
)?;

assert_eq!(packet.schema, "flight.ac7.telemetry/1");
# Ok::<(), flight_ac7_protocol::Ac7ProtocolError>(())
```
