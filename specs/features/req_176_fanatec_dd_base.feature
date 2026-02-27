@REQ-176 @product
Feature: Fanatec DD direct-drive base integration

  @AC-176.1
  Scenario: Fanatec DD1 steering wheel axis decoded
    Given a Fanatec DD1 direct-drive base with a wheel rim attached is connected
    When the steering wheel is rotated
    Then the wheel axis SHALL be decoded and available as a normalized flight control axis

  @AC-176.2
  Scenario: Fanatec CSL DD axis at center position reports zero
    Given a Fanatec CSL DD direct-drive base is connected
    When the steering wheel is held at its center position
    Then the wheel axis output SHALL be zero

  @AC-176.3
  Scenario: Steering angle range configurable via profile
    Given a profile specifying a custom steering angle range for the Fanatec DD base
    When the profile is loaded
    Then the wheel axis normalization SHALL use the configured angle range

  @AC-176.4
  Scenario: ClubSport V3 pedal module axes decoded
    Given a Fanatec ClubSport V3 pedal module is connected
    When the accelerator, brake, and clutch pedals are depressed
    Then all three pedal axes SHALL be decoded and available as normalized inputs

  @AC-176.5
  Scenario: Shifter connected as separate HID device
    Given a Fanatec shifter module is connected alongside the DD base
    When the shifter is operated
    Then it SHALL be enumerated and accessible as a separate HID device

  @AC-176.6
  Scenario: Force feedback output via proprietary protocol
    Given a Fanatec DD base with FFB enabled in the profile
    When a force effect is applied
    Then the force SHALL be transmitted to the device via the Fanatec proprietary FFB protocol

  @AC-176.7
  Scenario: Profile applied per connected wheel rim
    Given a Fanatec DD base with different wheel rims available in the profile
    When a specific wheel rim is connected
    Then the profile settings corresponding to that wheel rim SHALL be automatically applied
