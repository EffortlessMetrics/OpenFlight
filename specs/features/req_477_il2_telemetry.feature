@REQ-477 @product
Feature: IL-2 Sturmovik Telemetry — UDP Telemetry Processing  @AC-477.1
  Scenario: Adapter binds to configurable UDP port with default 29373
    Given an IL-2 adapter configuration specifying port 29373
    When the adapter starts
    Then it SHALL bind to UDP port 29373 and begin listening for telemetry frames  @AC-477.2
  Scenario: Telemetry frames are parsed and converted to BusSnapshot
    Given the IL-2 adapter is listening on its configured UDP port
    When a valid IL-2 telemetry frame is received
    Then the adapter SHALL parse the frame and publish a corresponding BusSnapshot to the bus  @AC-477.3
  Scenario: Missing or partial frames use last-known values with stale flag
    Given the IL-2 adapter has received at least one valid frame
    When a subsequent frame is missing or truncated
    Then the adapter SHALL publish the last known values with the stale flag set  @AC-477.4
  Scenario: Parse error rate is tracked in adapter metrics
    Given the IL-2 adapter is running and processing frames
    When frames with parse errors are received
    Then the adapter metrics SHALL record an incrementing parse error counter accessible via IPC
