@REQ-415 @product
Feature: Axis Value Histogram Output — Export Axis Histogram to ASCII Art

  @AC-415.1
  Scenario: flightctl axis histogram outputs ASCII bar chart to stdout
    Given a running or stopped service with recorded axis data
    When `flightctl axis histogram <axis_id>` is executed
    Then an ASCII bar chart SHALL be printed to stdout

  @AC-415.2
  Scenario: Bar chart shows 20 buckets covering [-1.0, 1.0]
    Given a histogram for any axis
    When the chart is rendered
    Then it SHALL display exactly 20 buckets spanning the range [-1.0, 1.0]

  @AC-415.3
  Scenario: Each bar shows relative frequency as percentage
    Given a non-empty histogram
    When the chart is displayed
    Then each bar SHALL show the relative frequency of that bucket as a percentage

  @AC-415.4
  Scenario: Output is human-readable without external tools
    Given the histogram output
    When it is viewed in a standard terminal
    Then it SHALL be fully human-readable without piping to any external tool

  @AC-415.5
  Scenario: Histogram command works even when service is stopped
    Given saved histogram data on disk
    When `flightctl axis histogram <axis_id>` is run while the service is stopped
    Then the command SHALL still produce output by reading the saved data

  @AC-415.6
  Scenario: Histogram export includes total sample count in header
    Given a rendered histogram
    When the header line is inspected
    Then it SHALL include the total sample count used to compute the histogram
