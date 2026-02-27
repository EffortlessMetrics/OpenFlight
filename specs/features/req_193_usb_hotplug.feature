@REQ-193 @product
Feature: Devices can be plugged and unplugged without restarting the service  @AC-193.1
  Scenario: New device detected within 500ms of connection
    Given the OpenFlight service is running with HID hot-plug monitoring active
    When a USB HID device is connected
    Then the service SHALL detect the new device within 500 milliseconds  @AC-193.2
  Scenario: Device removal detected within 500ms of disconnection
    Given a USB HID device is connected and active
    When the device is physically disconnected
    Then the service SHALL detect the removal within 500 milliseconds  @AC-193.3
  Scenario: Profile bindings reattach on device reconnect
    Given a device with profile bindings was previously connected and is now disconnected
    When the same device is reconnected
    Then the profile bindings SHALL reattach automatically without requiring a profile reload  @AC-193.4
  Scenario: Hot-plug event emitted to bus
    Given the event bus has a consumer subscribed to device events
    When a USB HID device is connected or disconnected
    Then a hot-plug event SHALL be emitted to the bus for downstream consumers  @AC-193.5
  Scenario: RT spine continues processing remaining axes during device swap
    Given multiple axes are active across multiple devices
    When one device is disconnected and a hot-plug event is being processed
    Then the RT spine SHALL continue processing axes from the remaining devices without interruption  @AC-193.6
  Scenario: Devices connected via HID hub detected correctly
    Given a USB HID hub with one or more devices attached
    When a device behind the hub is connected or disconnected
    Then the hot-plug detection SHALL recognise the event as if the device were directly connected
