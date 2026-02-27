@REQ-552 @product
Feature: SimConnect Managed Data Request — SimConnect adapter should use managed data requests for efficiency

  @AC-552.1
  Scenario: Managed data requests reduce per-frame SimConnect overhead
    Given the SimConnect adapter is using managed data requests
    When the adapter is profiled over 1000 frames
    Then per-frame SimConnect overhead SHALL be lower than with polled requests

  @AC-552.2
  Scenario: Data period is configurable per variable group
    Given a SimConnect variable group with a configured data period of 100ms
    When the adapter registers the managed request
    Then it SHALL set the SimConnect data period to 100ms for that group

  @AC-552.3
  Scenario: Changed-only updates are used for low-frequency data
    Given a variable group marked as low-frequency
    When the adapter registers the managed request
    Then it SHALL use the SIMCONNECT_PERIOD_VISUAL_FRAME with changed-only flag

  @AC-552.4
  Scenario: Managed request IDs are tracked and released on shutdown
    Given managed data requests have been registered
    When the service shuts down
    Then all managed request IDs SHALL be released via the SimConnect API
