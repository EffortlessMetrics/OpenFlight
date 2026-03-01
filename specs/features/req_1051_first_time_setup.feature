@REQ-1051 @product @user-journey
Feature: First-time setup experience
  As a new OpenFlight user
  I want a guided first-time setup
  So that I can start using my flight controls quickly without manual configuration

  @AC-1051.1
  Scenario: Fresh install creates default configuration directory
    Given OpenFlight has been freshly installed with no existing configuration
    When the service starts for the first time
    Then a default configuration directory SHALL be created at the platform-standard location
    And the directory SHALL contain a minimal global profile
    And the service SHALL log a "first_run_detected" event

  @AC-1051.2
  Scenario: Device discovery enumerates all connected HID flight devices
    Given the service is starting for the first time
    And a USB joystick and a USB throttle are connected
    When the first-time device discovery scan runs
    Then both devices SHALL be detected and listed by name and VID/PID
    And each device SHALL have its axis and button counts reported
    And a "devices_discovered" event SHALL be emitted on the bus

  @AC-1051.3
  Scenario: Default profile is generated from discovered devices
    Given the first-time device discovery has found a joystick with 3 axes and 12 buttons
    When the default profile generator runs
    Then a global profile SHALL be created with axis mappings for all discovered axes
    And the profile SHALL use conservative deadzone of 5% and linear response curves
    And the profile SHALL be saved to the configuration directory

  @AC-1051.4
  Scenario: Simulator detection identifies installed simulators
    Given the service is running first-time setup
    And MSFS 2020 is installed on the system
    When the simulator detection scan runs
    Then MSFS SHALL be identified as an available simulator
    And a simulator-specific profile template SHALL be offered for the detected sim
    And a "simulator_detected" event SHALL be emitted with the simulator identifier

  @AC-1051.5
  Scenario: Getting started wizard completes end-to-end setup
    Given the getting started wizard has been launched
    And one joystick and one simulator have been detected
    When the user completes the wizard accepting default settings
    Then a working profile SHALL be active for the detected device and simulator
    And the axis processing pipeline SHALL be running at 250 Hz
    And the wizard completion status SHALL be persisted so it is not shown again
