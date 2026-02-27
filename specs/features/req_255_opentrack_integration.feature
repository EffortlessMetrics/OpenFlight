@REQ-255 @product
Feature: OpenTrack head tracking data drives view offset axes  @AC-255.1
  Scenario: OpenTrack UDP protocol received on port 4242
    Given the OpenTrack adapter is configured and listening
    When an OpenTrack 48-byte 6DOF UDP packet arrives on port 4242
    Then the adapter SHALL decode the packet and forward the 6DOF values to the processing pipeline  @AC-255.2
  Scenario: Yaw pitch roll normalized to minus-one to one range
    Given a decoded OpenTrack packet with yaw/pitch/roll values
    When normalization is applied using the degree ranges 180/90/180
    Then the normalized yaw/pitch/roll values SHALL each be in the range [-1.0, 1.0]  @AC-255.3
  Scenario: Translation axes normalized to minus-one to one from 100mm range
    Given a decoded OpenTrack packet with x/y/z translation values in millimetres
    When normalization is applied using the ±100mm range
    Then the normalized x/y/z values SHALL each be in the range [-1.0, 1.0]  @AC-255.4
  Scenario: Stale data detected and output frozen after 500ms
    Given the OpenTrack adapter is receiving packets normally
    When no UDP packet is received for 500 milliseconds
    Then the adapter SHALL freeze all six output axes at their last valid values and set stale status  @AC-255.5
  Scenario: Six virtual axes exposed on bus
    Given the OpenTrack adapter is active
    When head tracking data is published
    Then the bus SHALL carry six virtual axes named head_yaw, head_pitch, head_roll, head_x, head_y, and head_z  @AC-255.6
  Scenario: OpenTrack pause state reflected in adapter status
    Given the OpenTrack adapter is connected
    When OpenTrack sends a pause or unpause signal
    Then the adapter status SHALL reflect the current OpenTrack pause/unpause state
