@REQ-274 @product
Feature: Config live reload applies updated profiles atomically without interrupting axis processing  @AC-274.1
  Scenario: Service detects profile file change within two seconds
    Given the service is running with a loaded profile
    When the profile file on disk is modified
    Then the service SHALL detect the change and begin processing the new profile within 2 seconds  @AC-274.2
  Scenario: New profile is validated before being applied
    Given the service is running with a loaded profile
    When a new profile file is written to disk
    Then the service SHALL validate the new profile against the schema before applying it  @AC-274.3
  Scenario: Invalid new profile does not replace current profile
    Given the service is running with a valid loaded profile
    When an invalid profile file is written to disk
    Then the service SHALL reject the invalid profile and continue using the previously loaded profile  @AC-274.4
  Scenario: Live reload can be disabled via config flag
    Given the service configuration has live reload disabled
    When the profile file on disk is modified
    Then the service SHALL NOT automatically reload the profile  @AC-274.5
  Scenario: Reload events are emitted on the health stream
    Given a gRPC client is subscribed to the health stream
    When a live profile reload occurs
    Then the health stream SHALL emit a reload event containing the new profile name  @AC-274.6
  Scenario: Reload does not interrupt running axis processing
    Given the RT axis processing spine is actively processing inputs at 250 Hz
    When a live profile reload is triggered
    Then the new profile SHALL be swapped in atomically at a tick boundary without dropping axis frames
