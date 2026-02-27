@REQ-190 @product
Feature: Live telemetry dashboard shows real-time axis values and metrics  @AC-190.1
  Scenario: Dashboard refreshes at 30Hz or faster
    Given the OpenFlight service is running and producing axis events on the bus
    When the live telemetry dashboard is open
    Then the displayed axis values SHALL update at a rate of at least 30 times per second  @AC-190.2
  Scenario: Tick jitter histogram visible in dashboard
    Given the dashboard is running and the RT spine has been ticking for at least one second
    When the jitter panel is viewed
    Then a histogram of RT spine tick jitter SHALL be visible and reflect current measurements  @AC-190.3
  Scenario: Device events shown in event log
    Given the dashboard is open and an event log panel is visible
    When a device is connected or disconnected while the dashboard is running
    Then the device connection or disconnection event SHALL appear in the event log  @AC-190.4
  Scenario: Per-axis FFB force displayed when FFB device connected
    Given a force feedback device is connected and the dashboard is open
    When the axis FFB panel is viewed
    Then the current force output for each FFB axis SHALL be displayed in real time  @AC-190.5
  Scenario: Dashboard accessible via CLI in text mode
    Given the OpenFlight service is running
    When the user runs flightctl monitor
    Then a text-mode live telemetry view SHALL be displayed in the terminal  @AC-190.6
  Scenario: Dashboard data exportable to JSON
    Given the dashboard is running with live telemetry data
    When the user requests a JSON export of the current dashboard state
    Then a well-formed JSON document containing all current axis values and metrics SHALL be written
