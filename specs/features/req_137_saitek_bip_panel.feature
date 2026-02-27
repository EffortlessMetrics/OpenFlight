@REQ-137 @product
Feature: Saitek BIP panel  @AC-137.1
  Scenario: BIP initializes all LEDs to Off
    Given a newly initialised Saitek BIP panel
    When all LED states are read
    Then every LED SHALL report the Off state  @AC-137.2
  Scenario: Set LED to green reads back green
    Given a Saitek BIP panel with all LEDs in the Off state
    When LED index 0 is set to the Green state
    Then reading LED index 0 SHALL return the Green state  @AC-137.3
  Scenario: Set LED to amber reads back amber
    Given a Saitek BIP panel with all LEDs in the Off state
    When LED index 3 is set to the Amber state
    Then reading LED index 3 SHALL return the Amber state  @AC-137.4
  Scenario: Set LED to red reads back red
    Given a Saitek BIP panel with all LEDs in the Off state
    When LED index 7 is set to the Red state
    Then reading LED index 7 SHALL return the Red state  @AC-137.5
  Scenario: Out-of-bounds set is silently ignored
    Given a Saitek BIP panel with 20 LEDs
    When a set operation is attempted for LED index 20
    Then no panic SHALL occur and no LED state SHALL change  @AC-137.6
  Scenario: Out-of-bounds get returns None
    Given a Saitek BIP panel with 20 LEDs
    When a get operation is attempted for LED index 20
    Then the result SHALL be None  @AC-137.7
  Scenario: encode_strip produces correct 25-byte output
    Given a BIP LED strip with a known pattern of Off Green Amber and Red states
    When encode_strip is called
    Then the returned byte slice SHALL be exactly 25 bytes and SHALL match the expected encoding  @AC-137.8
  Scenario: count_color counts correctly for mixed strip
    Given a BIP LED strip containing 3 Green 2 Amber and 1 Red LED and the remainder Off
    When count_color is called for the Green state
    Then the count SHALL be 3
