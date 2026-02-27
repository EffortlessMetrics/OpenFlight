@REQ-371 @product
Feature: Axis Output Range Mapping — Map Processed Value to Custom Output Range

  @AC-371.1
  Scenario: Output range is configurable per axis
    Given an axis with a custom output range [out_min, out_max] configured
    When the axis processes input values
    Then the output SHALL be mapped into the configured [out_min, out_max] range

  @AC-371.2
  Scenario: Default output range is [-1.0, 1.0]
    Given an axis with no explicit output range configured
    When the axis processes input values
    Then the output range SHALL default to [-1.0, 1.0]

  @AC-371.3
  Scenario: Mapping is linear between [-1.0, 1.0] input and output range
    Given an axis with output range [out_min, out_max] configured
    When an input of -1.0 is processed
    Then the output SHALL be out_min
    And when an input of 1.0 is processed
    Then the output SHALL be out_max

  @AC-371.4
  Scenario: Property test — output always stays within configured range
    Given an axis with an arbitrary output range [out_min, out_max]
    When arbitrary input values in [-1.0, 1.0] are processed via property testing
    Then every output value SHALL be within [out_min, out_max]

  @AC-371.5
  Scenario: Equal out_min and out_max produces constant output
    Given an axis with out_min equal to out_max (e.g. both 0.5)
    When any input value is processed
    Then the output SHALL always equal out_min

  @AC-371.6
  Scenario: No allocation occurs on the RT thread during range mapping
    Given an axis with output range mapping configured
    When range mapping runs on the RT thread
    Then no heap allocation SHALL occur
