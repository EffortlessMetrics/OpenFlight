# flight-cloud-profiles

Community profile repository client for Flight Hub.

Browse, download, publish, and vote on community-contributed flight control profiles.

## Features

- **List** profiles filtered by simulator, aircraft ICAO code, and free-text search
- **Download** a profile and cache it locally for offline use
- **Publish** a sanitized local profile to the community repository
- **Vote** (thumbs up/down) on any published profile
- **Disk cache** — profiles are cached under `~/.cache/flight-hub/cloud-profiles/` with configurable TTL

## CLI usage

```shell
# List top-rated community profiles
flightctl cloud-profiles list

# Filter by simulator
flightctl cloud-profiles list --sim msfs --sort newest

# Get a specific profile
flightctl cloud-profiles get abc123

# Publish a local profile
flightctl cloud-profiles publish ./my-profile.json --title "My A320 Setup" --description "Airbus sidestick curves"

# Vote on a profile
flightctl cloud-profiles vote abc123 up

# Clear the local cache
flightctl cloud-profiles clear-cache
```

## Library usage

```rust,no_run
use flight_cloud_profiles::{CloudProfileClient, ClientConfig, ListFilter};

# tokio_test::block_on(async {
let client = CloudProfileClient::new(ClientConfig::default())?;
let profiles = client.list(ListFilter::default()).await?;
for p in &profiles {
    println!("{}: {} (score: {})", p.id, p.title, p.score());
}
# Ok::<(), flight_cloud_profiles::CloudProfileError>(())
# });
```

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `base_url` | `https://profiles.flighthub.io/api/v1` | API base URL |
| `timeout` | 15 s | HTTP request timeout |
| `use_cache` | `true` | Enable local profile cache |

Set `FLIGHT_CLOUD_API_URL` in the environment to override the default endpoint.

## Privacy and safety

`sanitize_for_upload()` normalizes profiles before publishing:
- Resets the schema version field to the canonical value
- Lowercases the simulator slug
- Does **not** modify axis data — no personal identifiers are added or removed

`validate_for_publish()` enforces:
- Title must be 3–80 non-whitespace characters
- All axis bounds must be within `[-1.0, 1.0]`

## License

MIT OR Apache-2.0 — see [`LICENSE-MIT`](../../LICENSE-MIT) and [`LICENSE-APACHE`](../../LICENSE-APACHE).
