# T.Flight HOTAS 4 — HID Receipt Bundle

This directory holds raw HID captures for the Thrustmaster T.Flight HOTAS 4.
They are the ground truth for parser tests and fixture generation.

## Directory Layout

```
windows-driver/          Windows official Thrustmaster driver (PC mode)
  descriptor.bin         Raw HID report descriptor (if accessible)
  merged_reports.bin     Captured input reports in merged (8-byte) mode
  separate_reports.bin   Captured input reports in separate (9-byte) mode

linux-generic/           Linux kernel generic HID driver
  descriptor.bin
  merged_reports.bin
  separate_reports.bin

linux-hid-tflight4/      Linux with hid-tflight4 corrected descriptor module
  descriptor.bin
  merged_reports.bin
  separate_reports.bin

meta.json                Device metadata (VID/PID, firmware, capture tool)
```

## Status

⚠️ **Not yet captured.** Scaffold only.

Once a physical HOTAS 4 unit is available, capture receipts following the
procedure below, then drop the files into the appropriate subdirectory.

## Capture Procedure

### Linux (generic or hid-tflight4)

```bash
# Locate the sysfs descriptor node (adjust glob for your VID/PID)
DEVPATH=$(ls /sys/bus/hid/devices/*044F*B67*/report_descriptor 2>/dev/null | head -1)
cp "$DEVPATH" linux-generic/descriptor.bin
xxd -p linux-generic/descriptor.bin  # hex dump for inspection

# Capture raw input reports via hidraw (adjust /dev/hidrawN)
dd if=/dev/hidraw0 of=linux-generic/merged_reports.bin bs=9 count=100
```

### Windows (official driver)

Use USBPcap + Wireshark or the Thrustmaster joytester utility to export raw
report streams. Save as `merged_reports.bin` / `separate_reports.bin`.

## Using Receipts in Tests

Once captured, add binary fixtures under `crates/flight-hotas-thrustmaster/tests/fixtures/`
and update the unit tests tagged `// Scaffold` in `input.rs` with the real bytes.
