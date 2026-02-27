@REQ-136 @product
Feature: Saitek FIP panel integration  @AC-136.1
  Scenario: FIP display buffer is 320 by 240 pixels
    Given a newly initialised Saitek FIP display buffer
    When the buffer dimensions are queried
    Then the width SHALL be 320 pixels and the height SHALL be 240 pixels  @AC-136.2
  Scenario: Set pixel at 0 0 reads back correctly
    Given a Saitek FIP display buffer initialised to all zeros
    When the pixel at position 0 0 is set to RGB value 255 0 0
    Then reading the pixel at position 0 0 SHALL return RGB value 255 0 0  @AC-136.3
  Scenario: Set pixel at 319 239 reads back correctly
    Given a Saitek FIP display buffer initialised to all zeros
    When the pixel at position 319 239 is set to RGB value 0 255 0
    Then reading the pixel at position 319 239 SHALL return RGB value 0 255 0  @AC-136.4
  Scenario: RGB565 big-endian byte order is correct
    Given a pixel colour of R 31 G 63 B 31 encoded in RGB565 format
    When the two encoded bytes are inspected
    Then the high byte SHALL contain the upper bits and the low byte the lower bits per the RGB565 big-endian specification  @AC-136.5
  Scenario: Button Page1 press detected
    Given a Saitek FIP device with button event reporting enabled
    When the Page1 button is pressed
    Then a button-press event for Page1 SHALL be emitted by the panel driver  @AC-136.6
  Scenario: Button Rotary CW detected
    Given a Saitek FIP device with button event reporting enabled
    When the rotary encoder is turned one step clockwise
    Then a rotary-CW event SHALL be emitted by the panel driver  @AC-136.7
  Scenario: All-zero frame produces all-black display
    Given a Saitek FIP display buffer containing only zero bytes
    When the buffer is rendered to the device
    Then every visible pixel on the FIP display SHALL appear black  @AC-136.8
  Scenario: Out-of-bounds pixel write does not panic
    Given a Saitek FIP display buffer of 320 by 240 pixels
    When a write is attempted at pixel position 320 240
    Then no panic SHALL occur and the buffer contents SHALL remain unchanged
