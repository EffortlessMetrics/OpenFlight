# flight-integration-tests

End-to-end integration tests for the OpenFlight hardware parsing pipeline.

These tests verify the full flow from raw HID device reports through parsing, conversion,
and publication to the flight bus. They exercise multiple hardware crates together to catch
integration bugs that unit tests cannot.

## Running

```bash
cargo test -p flight-integration-tests
```

## Coverage

- VPForce Rhino HID report → bus snapshot
- Virpil HOTAS HID report → bus snapshot
- Thrustmaster T.Flight HID report → bus snapshot
- WinWing panel HID report → bus snapshot
- VKB STECS report → bus snapshot
