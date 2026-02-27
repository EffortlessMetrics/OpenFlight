Feature: AeroFly FS Native Plugin
  As a AeroFly FS user
  I want the AeroFly adapter to support native plugin communication
  So that I get lower latency input processing than UDP provides

  Background:
    Given the OpenFlight service is running

  Scenario: AeroFly plugin SDK protocol is documented in adapter guide
    When the AeroFly adapter guide is opened
    Then the plugin SDK protocol is documented with setup instructions

  Scenario: Native plugin mode provides lower latency than UDP
    Given both native plugin mode and UDP mode are available
    When latency is measured in both modes
    Then native plugin mode latency is lower than UDP mode latency

  Scenario: Native mode requires explicit configuration to enable
    Given the AeroFly adapter is configured without native mode setting
    When the adapter starts
    Then native plugin mode is not activated by default

  Scenario: Plugin mode availability is detected and logged
    Given the AeroFly adapter starts
    Then the log records whether native plugin mode is available
