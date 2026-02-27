@REQ-109
Feature: Device list and status RPC

  @AC-109.1
  Scenario: ListDevices response preserves all registered devices across a round-trip
    Given a device manager containing multiple registered devices
    When a ListDevices RPC response is encoded with prost and decoded back
    Then all registered devices SHALL be present in the decoded device list

  @AC-109.2
  Scenario: Removed device does not appear in subsequent device list queries
    Given a device manager where one device has been deregistered
    When the device list is queried
    Then the removed device SHALL not appear in the response

  @AC-109.3
  Scenario: Maximal device message with all capability and health fields survives a round-trip
    Given a Device message with every optional field including capabilities and health populated
    When the message is encoded with prost and decoded back
    Then all capability and health fields SHALL be preserved and equal to the originals
