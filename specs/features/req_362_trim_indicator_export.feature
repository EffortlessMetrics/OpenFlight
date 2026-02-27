@REQ-362 @product
Feature: Trim Indicator Export  @AC-362.1
  Scenario: Pitch, roll, and yaw trim positions are exported to MSFS L-Vars
    Given the trim export feature is enabled for MSFS
    When trim positions change for pitch, roll, or yaw
    Then the updated values SHALL be written to the corresponding MSFS L-Vars  @AC-362.2
  Scenario: Trim export happens within one RT tick of trim input change
    Given the trim export feature is enabled
    When a trim input change is processed by the RT spine
    Then the export to the sim SHALL occur within the same RT tick  @AC-362.3
  Scenario: Export can be enabled or disabled per simulator
    Given the system is connected to multiple simulators
    When trim export is enabled for one simulator and disabled for another
    Then only the enabled simulator SHALL receive trim export updates  @AC-362.4
  Scenario: Missing L-Var target is logged once and silently skipped
    Given trim export is enabled for MSFS and a configured L-Var does not exist
    When the service attempts to write to the missing L-Var
    Then it SHALL log a warning exactly once and skip all subsequent write attempts silently  @AC-362.5
  Scenario: Trim range is calibrated to the sim expected scale
    Given the trim export configuration specifies a target scale for the simulator
    When trim positions are exported
    Then the values SHALL be scaled to match the simulator expected range  @AC-362.6
  Scenario: Trim export is covered by an integration test with a mock sim
    Given the trim export integration test suite is executed with a mock simulator
    When the tests run
    Then all trim export scenarios SHALL pass against the mock sim
