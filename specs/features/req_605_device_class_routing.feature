Feature: Device Class Routing
  As a flight simulation enthusiast
  I want the service to route inputs based on device class
  So that appropriate default axis assignments are applied automatically

  Background:
    Given the OpenFlight service is running and HID devices are connected

  Scenario: Joysticks, throttles, pedals, and yokes are classified at enumeration
    When HID devices are enumerated at service startup
    Then each device is assigned a class of joystick, throttle, pedals, or yoke based on its descriptor

  Scenario: Class routing applies default axis assignments per class
    Given a throttle device is enumerated
    When no explicit axis assignments are configured for that device
    Then the default throttle class axis assignments are applied automatically

  Scenario: Class routing is overridable per device in profile
    Given a profile contains explicit axis assignments for a specific device
    When that device is enumerated
    Then the profile assignments take precedence over the class routing defaults

  Scenario: Device class is shown in flightctl devices output
    When the command "flightctl devices" is run
    Then the output includes the device class for each listed device
