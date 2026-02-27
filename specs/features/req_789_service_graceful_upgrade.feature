Feature: Service Graceful Upgrade
  As a flight simulation enthusiast
  I want service graceful upgrade
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Zero-downtime staged restart
    Given the system is configured for service graceful upgrade
    When the feature is exercised
    Then service supports zero-downtime upgrades via staged restart

  Scenario: Validate new version before shutdown
    Given the system is configured for service graceful upgrade
    When the feature is exercised
    Then new version is validated before old version shuts down

  Scenario: Transfer device state
    Given the system is configured for service graceful upgrade
    When the feature is exercised
    Then device state is transferred to the new service instance

  Scenario: Auto rollback on health failure
    Given the system is configured for service graceful upgrade
    When the feature is exercised
    Then upgrade rollback is automatic if the new version fails health check
