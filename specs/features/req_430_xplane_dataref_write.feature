@REQ-430 @product
Feature: X-Plane Dataref Write — Write Processed Axis Values Back as Datarefs

  @AC-430.1
  Scenario: Adapter maps processed axis values to X-Plane writable datarefs
    Given a profile with axis-to-dataref mappings configured
    When the axis engine produces a processed value
    Then the adapter SHALL write that value to the corresponding writable dataref

  @AC-430.2
  Scenario: Dataref write uses UDP commands on X-Plane 11+ writable interface
    Given the X-Plane adapter is connected via UDP
    When a dataref write is issued
    Then it SHALL use the X-Plane 11+ UDP DREF command on the configured port

  @AC-430.3
  Scenario: Write rate is configurable and defaults to 50 Hz
    Given no explicit write_rate_hz is set in config
    When the adapter is running
    Then dataref writes SHALL occur at 50 Hz
    And when write_rate_hz is set the adapter SHALL use that rate instead

  @AC-430.4
  Scenario: Failed writes are logged and counted in adapter metrics
    Given the X-Plane UDP socket returns a write error
    When a dataref write fails
    Then the failure SHALL be logged at WARN level and a failure counter SHALL be incremented
