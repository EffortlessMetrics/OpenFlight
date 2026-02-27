Feature: Hardware Agnostic FFB Profile
  As a flight simulation enthusiast
  I want FFB profiles to be hardware-agnostic where possible
  So that the same profile works across different force feedback devices

  Background:
    Given the OpenFlight service is running

  Scenario: FFB profile defines effects in physical units not device units
    When an FFB profile is authored
    Then all effect magnitudes and parameters are expressed in physical units

  Scenario: Device adapter translates physical units to device-specific values
    Given an FFB profile using physical units
    When the profile is applied to a specific FFB device
    Then the device adapter translates the physical values to device-specific units

  Scenario: Same profile produces equivalent experience on different FFB devices
    Given an FFB profile is loaded
    When the profile is applied to two different FFB devices
    Then both devices produce an equivalent force feedback experience

  Scenario: Translation mapping is documented per device in manifest
    When the device compatibility manifest is inspected
    Then it includes the physical-to-device unit translation mapping for each FFB device
