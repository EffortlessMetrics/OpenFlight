@REQ-575 @product
Feature: gRPC Service Reflection — gRPC server should support service reflection  @AC-575.1
  Scenario: gRPC reflection API is enabled in service
    Given the flightd service is running
    When a gRPC reflection request is sent to the server
    Then the server SHALL respond with the list of available services  @AC-575.2
  Scenario: CLI clients can discover available RPC methods
    Given the gRPC reflection API is enabled
    When a CLI client queries the reflection endpoint
    Then it SHALL receive a list of available RPC methods  @AC-575.3
  Scenario: Reflection is disabled in release builds by default
    Given the service is built in release mode with default feature flags
    When the service starts
    Then the gRPC reflection API SHALL not be enabled by default  @AC-575.4
  Scenario: flightctl help shows commands derived from reflection
    Given the flightd service is running with reflection enabled
    When the user runs flightctl help
    Then the output SHALL include commands discovered via gRPC reflection
