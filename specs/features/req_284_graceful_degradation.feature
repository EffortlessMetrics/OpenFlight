@REQ-284 @product
Feature: Graceful degradation with offline device marking, binding preservation, and health stream notification  @AC-284.1
  Scenario: Service continues operating with fewer axes if one device fails
    Given the service is running with two HID devices connected
    When one device is disconnected unexpectedly
    Then the service SHALL continue processing axes from the remaining device without restarting  @AC-284.2
  Scenario: Failed device is marked as offline in diagnostics
    Given a HID device that was previously online becomes unavailable
    When the diagnostics endpoint is queried
    Then the failed device SHALL appear with status offline in the device list  @AC-284.3
  Scenario: Profile bindings for failed device are preserved
    Given a profile with axis bindings assigned to a device that has gone offline
    When the profile configuration is inspected while the device is offline
    Then the axis bindings for the offline device SHALL remain present in the stored profile  @AC-284.4
  Scenario: When device reconnects bindings are restored automatically
    Given a device that went offline while the service was running
    When the device is reconnected to the USB bus
    Then the service SHALL detect the reconnection and restore the profile bindings for that device automatically  @AC-284.5
  Scenario: Degraded operation is logged with device details
    Given a device disconnects while the service is processing input
    When the degradation event is handled
    Then a structured log entry SHALL be written containing the device vendor ID, product ID, and disconnect reason  @AC-284.6
  Scenario: User is notified of degraded state via health stream
    Given a client is subscribed to the health stream gRPC endpoint
    When a connected device goes offline causing degraded operation
    Then the health stream SHALL emit a DegradedState message identifying the affected device
