@REQ-272 @product
Feature: Service applies structured error recovery with retry, fallback, backoff, and logging  @AC-272.1
  Scenario: Transient HID read error retried three times before disconnect
    Given a HID device that returns a transient read error
    When the error occurs during axis polling
    Then the service SHALL retry the read up to 3 times before marking the device as disconnected  @AC-272.2
  Scenario: Device reconnect restores previous profile binding
    Given a device that was disconnected while a specific profile was bound to it
    When the same device is reconnected
    Then the service SHALL automatically restore the profile binding that was active at disconnect  @AC-272.3
  Scenario: Simulator disconnect switches to idle profile not service stop
    Given a simulator adapter is active with a loaded profile
    When the simulator process terminates unexpectedly
    Then the service SHALL switch to the idle profile and continue running without stopping  @AC-272.4
  Scenario: Profile parse error falls back to last valid profile
    Given the service has a previously loaded valid profile
    When it attempts to load a new profile file that contains a parse error
    Then the service SHALL log the parse error and continue using the last successfully loaded profile  @AC-272.5
  Scenario: Recovery actions logged with error context
    Given an error recovery action is taken by the service
    When the recovery completes
    Then the log SHALL contain an entry that includes the original error description alongside the recovery action taken  @AC-272.6
  Scenario: Repeated failures trigger exponential backoff
    Given a device or adapter that fails repeatedly beyond the retry limit
    When successive failures continue to occur
    Then the service SHALL apply exponential backoff between reconnection attempts rather than retrying at a fixed rate
