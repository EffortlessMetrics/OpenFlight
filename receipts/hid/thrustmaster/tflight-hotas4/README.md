# T.Flight HOTAS 4 — HID Receipt Bundle

This directory holds raw HID captures for the Thrustmaster T.Flight HOTAS 4.
They are the ground truth for parser tests and fixture generation.

## Directory Layout

```
windows-driver/          Windows official Thrustmaster driver (PC mode)
  descriptor.bin         Raw HID report descriptor (if accessible)
  merged_reports.log     tflight_dump CSV output in merged (8-byte) mode
  separate_reports.log   tflight_dump CSV output in separate (9-byte) mode

linux-generic/           Linux kernel generic HID driver
  descriptor.bin
  merged_reports.log
  separate_reports.log

linux-hid-tflight4/      Linux with hid-tflight4 corrected descriptor module
  descriptor.bin
  merged_reports.log
  separate_reports.log

meta.json                Device metadata (VID/PID, firmware, capture tool)
```

## Status

⚠️ **Not yet captured.** Scaffold only.

Once a physical HOTAS 4 unit is available, use the procedure below, then drop
the files into the appropriate subdirectory.

## Fast Path: `tflight_dump` (recommended for initial receipts)

The `tflight_dump` example does everything you need — no daemon, no setup:

```sh
# Basic capture (stdout = CSV, stderr = diagnostics)
cargo run -p flight-hotas-thrustmaster --example tflight_dump \
  > receipts/hid/thrustmaster/tflight-hotas4/windows-driver/merged_reports.log

# If your stack prepends a Report ID byte (check: does the output show len=9 for merged?)
cargo run -p flight-hotas-thrustmaster --example tflight_dump \
  --strip-report-id \
  > receipts/hid/thrustmaster/tflight-hotas4/windows-driver/merged_reports.log

# Stop after 30 seconds
cargo run -p flight-hotas-thrustmaster --example tflight_dump \
  -- --duration=30 \
  > receipts/hid/thrustmaster/tflight-hotas4/windows-driver/merged_reports.log
```

Flip the HOTAS 4 mode switch mid-capture: you should see `len=8` (merged) and
`len=9` (separate) transitions in the log — the parser auto-detects them.

## Capture Procedure

### Linux (generic or hid-tflight4)

```bash
# Locate the sysfs descriptor node (adjust glob for your VID/PID)
DEVPATH=$(ls /sys/bus/hid/devices/*044F*B67*/report_descriptor 2>/dev/null | head -1)
cp "$DEVPATH" linux-generic/descriptor.bin
xxd -p linux-generic/descriptor.bin  # hex dump for inspection

# Capture via tflight_dump
cargo run -p flight-hotas-thrustmaster --example tflight_dump -- --duration=30 \
  > receipts/hid/thrustmaster/tflight-hotas4/linux-generic/merged_reports.log
```

### Windows (official driver)

```sh
# Capture via tflight_dump (run from workspace root)
cargo run -p flight-hotas-thrustmaster --example tflight_dump -- --duration=30 \
  > receipts\hid\thrustmaster\tflight-hotas4\windows-driver\merged_reports.log
```

For the raw descriptor on Windows, use USBPcap + Wireshark or export from
Device Manager → HID descriptor.

## Using Receipts in Tests

Once captured:
1. Extract a handful of representative lines from the CSV log.
2. Convert `raw_hex` column entries to `&[u8]` byte literals.
3. Replace the `// Scaffold` fixture constants in
   `crates/flight-hotas-thrustmaster/src/input.rs` with the real bytes.
4. Re-run `cargo test -p flight-hotas-thrustmaster --lib` to lock them in.

