@REQ-497 @product
Feature: Device Hotplug Notification — Real-Time Plug/Unplug Event Display  @AC-497.1
  Scenario: flightctl monitor shows real-time device plug/unplug events
    Given `flightctl monitor` is running
    When a HID device is connected or disconnected
    Then the event SHALL appear in the monitor output in real time  @AC-497.2
  Scenario: Events include device name, VID/PID, and timestamp
    Given a device plug event is reported by flightctl monitor
    When the event is inspected
    Then it SHALL include the device name, VID, PID, and an ISO-8601 timestamp  @AC-497.3
  Scenario: Known devices are identified by name from compat manifests
    Given a device whose VID/PID appears in a compat manifest
    When it is connected and reported by flightctl monitor
    Then the event SHALL show the human-readable device name from the manifest  @AC-497.4
  Scenario: Hotplug events are also published on flight-bus
    Given the service is running and a HID device is connected
    When the hotplug event is detected
    Then a corresponding hotplug event SHALL be published on flight-bus
