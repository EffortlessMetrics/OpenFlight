@REQ-247 @product
Feature: FFB force output clamped by safety interlocks to rated limits  @AC-247.1
  Scenario: Per-device rated force limit read from device manifest
    Given a force feedback device with a device manifest declaring a rated force limit
    When the FFB driver initialises the device
    Then the rated force limit from the manifest SHALL be loaded and used as the clamp ceiling for that device  @AC-247.2
  Scenario: Output force clamped to rated limit before sending to device
    Given a device with a rated force limit of 10 N and a requested output of 15 N
    When the safety interlock processes the output command
    Then the value sent to the device SHALL be clamped to 10 N  @AC-247.3
  Scenario: Clamp events counted and available in capability report
    Given a device that has had three force outputs clamped during the current service session
    When a GetCapabilities gRPC call is made
    Then the capability report SHALL include a clamp_event_count of 3 for that device  @AC-247.4
  Scenario: Emergency stop command immediately zeros all FFB output
    Given one or more FFB devices actively outputting force
    When an emergency stop command is issued
    Then all FFB device outputs SHALL be set to zero within one RT tick  @AC-247.5
  Scenario: FFB envelope enforced ramp-up rate limited to prevent shock loads
    Given a device with a configured maximum ramp-up rate
    When a step-change force command is issued that exceeds the ramp-up limit
    Then the interlock SHALL ramp the output at the maximum permitted rate rather than applying the step change immediately  @AC-247.6
  Scenario: Safety interlock tests run in CI before any FFB code merge
    Given the CI pipeline is configured for a pull request touching flight-ffb source
    When the pipeline executes
    Then the FFB safety and envelope test suites SHALL run and a failure in either SHALL block the merge
