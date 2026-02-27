@REQ-387 @product
Feature: X-Plane Plugin Install Helper  @AC-387.1
  Scenario: Plugin is copied to X-Plane Resources/plugins via flightctl
    Given X-Plane is installed and its path is configured in the service
    When the user runs flightctl xplane install-plugin
    Then the OpenFlight plugin SHALL be copied to X-Plane Resources/plugins  @AC-387.2
  Scenario: Plugin provides richer telemetry than standard UDP datarefs
    Given the OpenFlight X-Plane plugin is installed and the service is connected
    When telemetry data is read from the plugin
    Then it SHALL provide shared memory or UDP telemetry richer than standard UDP datarefs  @AC-387.3
  Scenario: Plugin version mismatch produces a warning on connect
    Given the installed plugin version differs from the expected service version
    When the service connects to X-Plane with the mismatched plugin
    Then a version mismatch warning SHALL be logged and reported to the user  @AC-387.4
  Scenario: Plugin install is reversible via flightctl xplane remove-plugin
    Given the OpenFlight plugin is installed in X-Plane Resources/plugins
    When the user runs flightctl xplane remove-plugin
    Then the plugin files SHALL be removed from X-Plane Resources/plugins  @AC-387.5
  Scenario: Plugin install fails gracefully if X-Plane path is not configured
    Given no X-Plane installation path is configured in service settings
    When the user runs flightctl xplane install-plugin
    Then the command SHALL fail with a clear error message about the missing path  @AC-387.6
  Scenario: Plugin compatibility matrix lists supported X-Plane versions
    Given the plugin compatibility configuration
    When supported X-Plane versions are queried
    Then X-Plane 11 and X-Plane 12 SHALL be listed as supported versions
