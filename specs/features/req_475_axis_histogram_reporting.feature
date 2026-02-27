@REQ-475 @product
Feature: Axis Histogram Reporting — Value Distribution Diagnostics  @AC-475.1
  Scenario: Histogram records value distribution across 100 equal-width buckets
    Given an axis is actively processing input values
    When sufficient samples have been collected
    Then the histogram SHALL contain exactly 100 equal-width buckets covering the full value range  @AC-475.2
  Scenario: Histogram is accessible via flightctl axis histogram command
    Given the service is running and an axis has collected histogram data
    When `flightctl axis histogram <axis_id>` is executed
    Then the command SHALL return the histogram data for the specified axis  @AC-475.3
  Scenario: ASCII histogram is displayed in terminal with frequency bars
    Given histogram data is available for an axis
    When `flightctl axis histogram <axis_id>` is executed in a terminal
    Then the output SHALL render an ASCII bar chart with bucket ranges and relative frequencies  @AC-475.4
  Scenario: Histogram data can be exported as JSON for further analysis
    Given histogram data is available for an axis
    When `flightctl axis histogram <axis_id> --format json` is executed
    Then the output SHALL be valid JSON containing bucket boundaries and counts
