@REQ-485 @product
Feature: Boot-Time Device Check — Startup Device Presence Verification  @AC-485.1
  Scenario: Startup check lists required devices from active profile
    Given an active profile that declares required devices
    When the service starts
    Then the service SHALL enumerate and log the required devices listed in the active profile  @AC-485.2
  Scenario: Missing required devices are logged as warnings
    Given an active profile that declares a required device that is not connected
    When the service starts
    Then the service log SHALL contain a warning identifying the missing required device  @AC-485.3
  Scenario: Service continues with reduced functionality if optional devices absent
    Given an active profile with an optional device that is not connected
    When the service starts
    Then the service SHALL start successfully and operate with the optional device's features disabled  @AC-485.4
  Scenario: Boot check results are included in flightctl status output
    Given the service has completed startup with some devices absent
    When `flightctl status` is executed
    Then the output SHALL include a section summarising the boot device check results
