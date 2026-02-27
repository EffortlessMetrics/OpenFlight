# flight-metrics-http

Minimal Prometheus-compatible HTTP metrics endpoint for OpenFlight services.

Exposes a `/metrics` endpoint that returns OpenMetrics-format text for scraping by Prometheus, Grafana Agent, or compatible collectors.

## Usage

Add to your service and call `MetricsServer::start()` with a bind address. The server runs on a background thread and does not block the RT spine.

## License

Licensed under MIT OR Apache-2.0.
