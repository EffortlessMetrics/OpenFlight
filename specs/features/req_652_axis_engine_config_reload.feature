Feature: Axis Engine Configuration Reload
  As a flight simulation enthusiast
  I want the axis engine to support live configuration reload
  So that configuration changes take effect without restarting the service

  Background:
    Given the OpenFlight service is running with an active axis engine configuration

  Scenario: Engine configuration can be reloaded without restart
    Given a new axis engine configuration has been prepared
    When the command "flightctl axis reload" is run
    Then the axis engine applies the new configuration without restarting

  Scenario: Reload is atomic — either full old or full new config applies
    Given a reload is in progress
    When the new configuration is swapped in
    Then at no point is a partial mix of old and new configuration active

  Scenario: Failed reload reverts to previous config
    Given a new configuration contains an invalid axis curve definition
    When a reload is attempted
    Then the reload fails and the previous valid configuration remains active

  Scenario: Reload events are counted in engine diagnostics
    Given several successful and failed reloads have occurred
    When engine diagnostics are retrieved
    Then the output includes the count of successful and failed reload events
