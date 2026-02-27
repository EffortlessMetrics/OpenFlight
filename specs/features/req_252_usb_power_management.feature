@REQ-252 @product
Feature: OpenFlight handles USB device power states without dropping inputs  @AC-252.1
  Scenario: USB selective suspend does not cause axis data loss
    Given a HID device that enters USB selective suspend during an idle period
    When the device resumes and sends a new input report
    Then no axis input reports SHALL be dropped and the axis value SHALL update correctly  @AC-252.2
  Scenario: Device wake-up detected within 200ms
    Given a HID device currently in a suspended USB power state
    When the device wakes up and resumes sending reports
    Then the driver SHALL detect the wake-up within 200 milliseconds  @AC-252.3
  Scenario: Wireless HID receiver dongle treated as persistent connection
    Given a wireless device whose USB dongle remains plugged in while the wireless controller is off
    When the wireless controller is switched off
    Then the dongle SHALL remain registered as a persistent connection and NOT trigger a device-removed event  @AC-252.4
  Scenario: USB 3.x hub compatibility device polling works at full rate
    Given a HID device connected through a USB 3.x hub
    When axis data is being polled
    Then the polling rate SHALL match the device's configured rate with no hub-induced throttling  @AC-252.5
  Scenario: Battery-powered device low battery event forwarded to bus
    Given a wireless HID device reporting a low battery status in its input report
    When the driver processes the report
    Then a low-battery event SHALL be emitted to the flight bus including the device ID  @AC-252.6
  Scenario: Power state transitions logged at DEBUG level
    Given a HID device that transitions between active and suspended USB power states
    When the power state change is detected
    Then the transition SHALL be logged at DEBUG level including the device ID and new state
