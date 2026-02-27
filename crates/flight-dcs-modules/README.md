# flight-dcs-modules

DCS World aircraft module loader for OpenFlight.

## Overview

Drop one `.toml` file per aircraft into a modules directory and use
`ModuleLoader` to read them at startup. Each file describes the axis count,
throttle range, stick throw, and any known behavioural quirks for that
DCS module.

## Module format

```toml
aircraft = "F/A-18C"
axis_count = 6
throttle_range = [0.0, 1.0]   # [min, max] normalised
stick_throw = 45.0             # degrees, centre to stop
quirks = ["twin-throttle", "catapult-bar"]
```

## Sample modules

Three bundled examples live in the `modules/` directory:

| File | Aircraft |
|------|----------|
| `fa-18c.toml` | F/A-18C Hornet |
| `f-16c.toml`  | F-16C Viper    |
| `a-10c.toml`  | A-10C Warthog  |

## License

MIT OR Apache-2.0
