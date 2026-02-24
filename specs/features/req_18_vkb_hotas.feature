@REQ-18
Feature: VKB HOTAS input parsing and virtual controller aggregation

  @AC-18.1
  Scenario: Parse STECS interface report with axes and buttons
    Given a STECS input handler for the RightSpaceThrottleGripStandard variant
    And a 14-byte HID report with known axis values and button bits
    When the report is parsed
    Then all five axes SHALL be decoded to their expected normalized values
    And the button bitmask SHALL match the expected 32-bit pattern

  @AC-18.1
  Scenario: Parse STECS interface report with buttons only
    Given a STECS input handler for the LeftSpaceThrottleGripMini variant
    And a 4-byte HID report with button data only
    When the report is parsed
    Then the parsed state SHALL contain no axes
    And the button bitmask SHALL reflect the pressed button

  @AC-18.1
  Scenario: Parse STECS report with prepended Report ID byte
    Given a STECS input handler with report_id mode enabled
    And a 15-byte HID report with a Report ID prefix
    When the report is parsed
    Then the Report ID SHALL be stripped and axes SHALL be decoded correctly

  @AC-18.2
  Scenario: Merge virtual controller reports into global button range
    Given a STECS aggregator for the RightSpaceThrottleGripMiniPlus variant
    When reports are submitted for VC0, VC1, and VC2 with distinct button bits
    Then the merged state SHALL map VC0 buttons to 1-32, VC1 to 33-64, and VC2 to 65-96

  @AC-18.2
  Scenario: Lowest-indexed virtual controller axes take precedence
    Given a STECS aggregator receiving axis data from VC0 and VC1
    When both VCs report different axis values
    Then the merged axes SHALL reflect VC0 values

  @AC-18.3
  Scenario: Report virtual controller index out of range error
    Given a STECS aggregator
    When a report is submitted for virtual controller index 3
    Then a VirtualControllerOutOfRange error SHALL be returned

  @AC-18.3
  Scenario: Report too-short HID report error
    Given a STECS input handler expecting at least 4 bytes
    When a 3-byte report is presented
    Then a ReportTooShort error SHALL be returned with expected and actual lengths

  @AC-18.4
  Scenario: Health monitor tracks failure threshold
    Given a STECS health monitor with default threshold of 3
    When 3 consecutive failures are recorded
    Then is_failed SHALL return true
    And recording a success SHALL reset the failure count

  @AC-18.4
  Scenario: Health status reflects connected and healthy device
    Given a STECS health monitor with zero failures
    When status is queried with connected=true
    Then is_healthy SHALL return true
