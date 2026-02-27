@REQ-157 @product
Feature: Stream Deck integration  @AC-157.1
  Scenario: Key press events are received from Stream Deck
    Given a Stream Deck device is connected and initialised
    When a physical key is pressed
    Then a key-press event SHALL be raised with the correct key index  @AC-157.2
  Scenario: Key image is set via USB HID output report
    Given a Stream Deck device is connected
    When a key image write command is issued for a specific key index
    Then the image data SHALL be transmitted via the USB HID output report  @AC-157.3
  Scenario: Brightness is set correctly across the 0 to 100 percent range
    Given a Stream Deck device is connected
    When a brightness command is issued for each boundary value 0 and 100
    Then the device SHALL receive the corresponding brightness HID report  @AC-157.4
  Scenario: Multi-key press combination is handled
    Given a Stream Deck device is connected
    When multiple keys are pressed simultaneously
    Then each simultaneous key-press SHALL produce an independent event  @AC-157.5
  Scenario: Disconnect and reconnect are handled cleanly
    Given a Stream Deck device is connected and in use
    When the device is disconnected and then reconnected
    Then the driver SHALL re-initialise the device and resume normal operation without error  @AC-157.6
  Scenario: Key count is correct for each Stream Deck model
    Given Stream Deck Original, XL, and Mini devices are individually connected
    When the device model is identified
    Then the key counts SHALL be 15 for Original, 32 for XL, and 6 for Mini  @AC-157.7
  Scenario: LED update batch write is efficient
    Given a Stream Deck device is connected
    When images for all keys are updated in a single batch operation
    Then the batch SHALL complete within the allowed HID write budget  @AC-157.8
  Scenario: Profile switching triggers key image update
    Given a Stream Deck device is connected with a loaded profile
    When the active profile is switched
    Then key images on the Stream Deck SHALL be updated to reflect the new profile
