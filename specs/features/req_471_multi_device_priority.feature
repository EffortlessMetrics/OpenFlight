@REQ-471 @product
Feature: Multi-Device Priority Ordering — Device Priority for Shared Axes  @AC-471.1
  Scenario: Priority order is configurable per virtual axis
    Given a profile with two devices mapped to the same virtual axis
    When the priority order is set in the axis configuration
    Then the configured priority order SHALL be persisted and applied on next tick  @AC-471.2
  Scenario: Highest-priority active device value is used by default
    Given two devices mapped to the same virtual axis with different priorities
    When both devices report values simultaneously
    Then the output SHALL use the value from the highest-priority device  @AC-471.3
  Scenario: Priority mode can be set to override, blend, or sum
    Given a virtual axis with two device inputs
    When the priority mode is set to "blend"
    Then the output SHALL be a weighted blend of both device values rather than the highest-priority only  @AC-471.4
  Scenario: Priority order changes take effect without service restart
    Given the service is running with a priority configuration
    When the priority order is changed via flightctl or config hot-reload
    Then the new priority order SHALL take effect on the next tick without restarting the service
