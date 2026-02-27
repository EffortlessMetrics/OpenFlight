@REQ-404 @product
Feature: Axis Histogram Recording — Collect Value Distribution Statistics

  @AC-404.1
  Scenario: Optional histogram tracks value frequency in 100 equal-width buckets
    Given an axis with histogram recording enabled
    When axis values are processed over multiple ticks
    Then the histogram SHALL track frequency across 100 equal-width buckets spanning [-1.0, 1.0]

  @AC-404.2
  Scenario: Histogram updates on the RT thread without allocation
    Given histogram recording is active on an axis
    When a tick is processed on the RT thread
    Then the histogram update SHALL complete without heap allocation

  @AC-404.3
  Scenario: Histogram is readable via flightctl axis histogram
    Given an axis with histogram data collected
    When the user runs `flightctl axis histogram <axis_id>`
    Then the current histogram data SHALL be returned

  @AC-404.4
  Scenario: Histogram resets when calibration is reset
    Given an axis with existing histogram data
    When the axis calibration is reset
    Then all histogram bucket counts SHALL be cleared to zero

  @AC-404.5
  Scenario: Histogram allows detection of axis dead zones and saturation zones
    Given a histogram with concentrated counts in low-value or extreme buckets
    When the user inspects the histogram
    Then dead zone and saturation patterns SHALL be identifiable

  @AC-404.6
  Scenario: Histogram is exported as a human-readable ASCII bar chart
    Given an axis with collected histogram data
    When the histogram is exported
    Then the output SHALL be a human-readable ASCII bar chart representation
