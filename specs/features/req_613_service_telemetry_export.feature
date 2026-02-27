Feature: Service Telemetry Export
  As a flight simulation enthusiast
  I want to export service telemetry to external systems
  So that I can monitor and analyse OpenFlight performance

  Background:
    Given the OpenFlight service is running

  Scenario: Telemetry can be exported via OpenTelemetry OTLP protocol
    Given an OTLP endpoint is configured
    When the service is running
    Then telemetry is exported to the OTLP endpoint

  Scenario: Export endpoint is configurable in service config
    When the user sets the OTLP endpoint in the service config
    Then the service exports telemetry to the configured endpoint

  Scenario: Export includes axis, device, and adapter metrics
    Given telemetry export is enabled
    When telemetry is exported
    Then the export contains axis metrics, device metrics, and adapter metrics

  Scenario: Export can be disabled to reduce overhead
    Given telemetry export is enabled
    When the user sets telemetry_export_enabled to false in the service config
    Then the service stops exporting telemetry
