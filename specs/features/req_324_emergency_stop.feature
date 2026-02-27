@REQ-324 @product
Feature: Emergency Stop  @AC-324.1
  Scenario: Service supports a hardware emergency stop button mapping
    Given a button on a connected HID device is mapped as the emergency stop
    When that button is pressed
    Then the service SHALL trigger the emergency stop procedure  @AC-324.2
  Scenario: All axes are forced to neutral when E-stop is triggered
    Given the service is processing active axis inputs
    When the emergency stop is triggered
    Then all axis outputs SHALL immediately be set to their neutral (centre) value  @AC-324.3
  Scenario: E-stop state is maintained until explicitly cleared
    Given the emergency stop has been triggered
    When subsequent axis input is received
    Then the service SHALL continue to output neutral values until the E-stop is explicitly cleared  @AC-324.4
  Scenario: E-stop event is logged with timestamp and triggering button
    Given the emergency stop is triggered by button ID 7 on device VID/PID 0x1234/0x5678
    When the event is processed
    Then the service log SHALL contain an entry with the UTC timestamp, device identifier, and button ID  @AC-324.5
  Scenario: E-stop clears all active FFB effects immediately
    Given force feedback effects are active on connected devices
    When the emergency stop is triggered
    Then all active FFB effects SHALL be cancelled immediately  @AC-324.6
  Scenario: CLI can trigger and clear E-stop
    Given the service is running
    When the user runs flightctl estop to trigger and flightctl estop --clear to release
    Then the service SHALL enter E-stop state on the first command and return to normal operation on the second
