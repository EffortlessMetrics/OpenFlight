# flight-hotas-brunner

Brunner Elektronik AG CLS-E Force Feedback Yoke device support for [OpenFlight](https://github.com/flight-hub/openflight).

## Supported devices

| Device | VID | PID | Report bytes | Support tier |
|---|---|---|---|---|
| Brunner CLS-E FFB Yoke | 0x25BB | 0x0063 | 9 | 3 |

## VID/PID source

- VID 0x25BB confirmed: [linux-usb.org USB ID registry](https://www.linux-usb.org/usb-ids.html) (Brunner Elektronik AG)
- PID 0x0063 confirmed: [the-sz.com USB ID database](https://www.the-sz.com/products/usbid/), listed as "PRT.5105 [Yoke]" — the Brunner part number for the CLS-E USB interface.

## Report format

```text
byte  0     : report_id (0x01)
bytes 1–2   : roll  / X axis (i16 LE, bipolar: −32768…+32767)
bytes 3–4   : pitch / Y axis (i16 LE, bipolar: −32768…+32767)
bytes 5–8   : button bytes (32 buttons, LSB-first across 4 bytes)
total: 9 bytes minimum
```

Axis values are normalised to −1.0…+1.0.

## Notes

Report layout is inferred from Brunner SDK documentation and USB registry data.
Hardware-in-the-loop (HIL) validation has **not** been performed. This crate handles
HID input parsing only — FFB output requires the Brunner CLS2SIM companion software
or the Brunner SDK.
