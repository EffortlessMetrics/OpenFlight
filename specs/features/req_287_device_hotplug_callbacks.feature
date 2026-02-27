@REQ-287 @product
Feature: Device hotplug callbacks with 200ms latency, multi-listener support, VID/PID payload, and auto-cleanup  @AC-287.1
  Scenario: Service registers a hotplug callback via HID API
    Given the service is starting up
    When HID device monitoring is initialised
    Then the service SHALL have registered a hotplug callback with the underlying HID API  @AC-287.2
  Scenario: Callback fires within 200ms of USB plug or unplug
    Given the service is running with hotplug monitoring active
    When a USB HID device is connected or disconnected
    Then the registered hotplug callback SHALL be invoked within 200 milliseconds of the event  @AC-287.3
  Scenario: Multiple listeners can register for hotplug events
    Given two independent subsystems have each registered a hotplug listener
    When a device is connected
    Then both listeners SHALL receive the hotplug notification for that event  @AC-287.4
  Scenario: Callback includes device VID/PID and event type
    Given a listener is registered for hotplug events
    When a device connect event fires
    Then the callback payload SHALL include the device vendor ID, product ID, and an event type of connected or disconnected  @AC-287.5
  Scenario: Callbacks are invoked from a dedicated thread not the RT spine
    Given the hotplug monitoring system is active
    When a hotplug callback is triggered
    Then the callback SHALL execute on the dedicated hotplug thread and SHALL NOT run on the RT processing thread  @AC-287.6
  Scenario: Stale callbacks are automatically cleaned up
    Given a subsystem registered a hotplug listener and has since been dropped
    When a subsequent hotplug event fires
    Then the stale callback entry SHALL be removed and no callback SHALL be invoked for the dropped subsystem
