@REQ-65
Feature: HID device management

  @AC-65.1
  Scenario: Device registration and unregistration update the registry
    Given an empty HID device registry
    When a device is registered and then unregistered
    Then the registry SHALL reflect each state correctly

  @AC-65.2
  Scenario: Endpoint operations update adapter statistics
    Given a HID adapter with registered endpoints
    When read and write operations are performed
    Then per-operation statistics SHALL be updated and interface metadata SHALL be accessible

  @AC-65.3
  Scenario: HID write errors have correct display strings
    Given the set of HID writer error variants
    When each error is formatted as a display string
    Then each string SHALL match the expected human-readable description

  @AC-65.4
  Scenario: Fault detection enters faulted state at the failure threshold
    Given a HID fault-detection tracker with a consecutive-failure threshold of N
    When exactly N consecutive failures are recorded
    Then the tracker SHALL transition to the faulted state

  @AC-65.5
  Scenario: Faulted HID writer recovers on a successful write
    Given a HID fault-detection tracker already in the faulted state
    When a successful write is recorded
    Then the tracker SHALL leave the faulted state and reset the failure counter
