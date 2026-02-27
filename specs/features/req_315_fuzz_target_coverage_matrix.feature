@REQ-315 @product
Feature: Fuzz Target Coverage Matrix  @AC-315.1
  Scenario: Each parser crate has at least one fuzz target
    Given the list of parser crates including HOTAS and simulator adapters
    When fuzz targets are enumerated for each crate
    Then every parser crate SHALL have at least one dedicated fuzz target  @AC-315.2
  Scenario: Fuzz corpus is seeded with real-world HID report examples
    Given the fuzz target corpus directories
    When the fuzz corpus is inspected
    Then each fuzz target corpus SHALL contain seed inputs derived from real-world HID report captures  @AC-315.3
  Scenario: Coverage is reported per fuzz target
    Given a CI fuzz run completes
    When coverage data is collected
    Then the CI SHALL report code coverage independently for each fuzz target  @AC-315.4
  Scenario: All fuzz targets compile without feature flag changes
    Given the workspace in its default feature configuration
    When fuzz targets are compiled
    Then all fuzz targets SHALL compile successfully without requiring any additional --features arguments  @AC-315.5
  Scenario: Fuzz manifest entries in COMPATIBILITY.md show fuzz coverage column
    Given the COMPATIBILITY.md file
    When the fuzz coverage section is reviewed
    Then each fuzz target entry SHALL include a column indicating its fuzz coverage status  @AC-315.6
  Scenario: CI nightly fuzz run lasts minimum 60 seconds per target
    Given the nightly CI fuzz job configuration
    When the fuzz run executes
    Then each fuzz target SHALL be run for a minimum of 60 seconds during the nightly CI job
