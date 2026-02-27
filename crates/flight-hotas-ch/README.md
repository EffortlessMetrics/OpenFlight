# flight-hotas-ch

CH Products device support helpers for OpenFlight.

Provides recommended axis presets and health monitoring for CH Products HOTAS devices
(Fighterstick, Combat Stick, Pro Throttle, Pro Pedals, Eclipse Yoke, Flight Yoke).

CH Products devices use OS-mediated HID (standard HID descriptor), so no raw byte
parser is required — axis and button data are delivered by the OS HID stack directly.
