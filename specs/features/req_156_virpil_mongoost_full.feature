@REQ-156 @product
Feature: VIRPIL MongoosT-50CM3 full integration  @AC-156.1
  Scenario: MongoosT XY axes report centre position with 14-bit resolution
    Given the VIRPIL MongoosT-50CM3 is connected with VID 0x3344
    When both X and Y axes are at mechanical centre
    Then the decoded axis values SHALL each be 0.0 with 14-bit precision  @AC-156.2
  Scenario: Roll axis reports maximum deflection
    Given the VIRPIL MongoosT-50CM3 is connected
    When the roll axis is deflected to its physical maximum
    Then the decoded roll axis value SHALL be 1.0  @AC-156.3
  Scenario: Throttle axis is decoded
    Given the VIRPIL MongoosT-50CM3 is connected
    When the throttle axis is moved across its full travel
    Then the decoded throttle axis SHALL report values across the full range  @AC-156.4
  Scenario: Side throttle is decoded independently
    Given the VIRPIL MongoosT-50CM3 with side throttle is connected
    When the side throttle is moved across its full travel
    Then the side throttle SHALL map to a distinct decoded axis  @AC-156.5
  Scenario: All 46 buttons are decoded correctly
    Given the VIRPIL MongoosT-50CM3 is connected
    When each of the 46 buttons is pressed individually
    Then each button event SHALL carry the correct button index and pressed state  @AC-156.6
  Scenario: Mode selector position is decoded
    Given the VIRPIL MongoosT-50CM3 is connected
    When the mode selector is rotated to each position
    Then each position SHALL produce a distinct decoded axis or button event  @AC-156.7
  Scenario: 5-way China Hat is decoded
    Given the VIRPIL MongoosT-50CM3 is connected
    When the China Hat is pressed in each of the 5 directions
    Then each direction SHALL produce a distinct decoded button event  @AC-156.8
  Scenario: Device is recognised by VID
    Given a HID enumeration is performed
    When a device with VID 0x3344 is found matching the MongoosT-50CM3 PID
    Then it SHALL be identified as a VIRPIL MongoosT-50CM3 stick
