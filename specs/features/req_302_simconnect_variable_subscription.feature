@REQ-302 @product
Feature: SimConnect Variable Subscription

  @AC-302.1
  Scenario: Service subscribes to SimConnect variables defined in profile
    Given a profile that lists one or more SimConnect simulation variables
    When the service starts and connects to MSFS
    Then the service SHALL subscribe to each variable listed in the profile via the SimConnect API

  @AC-302.2
  Scenario: Variable subscription uses efficient SimConnect event-based API
    Given the service is connected to MSFS via SimConnect
    When simulation variable data is requested
    Then the service SHALL use SimConnect data-request callbacks rather than polling

  @AC-302.3
  Scenario: New variable subscriptions are added without restart
    Given the service is running and connected to MSFS
    When the active profile is updated to add a new SimConnect variable
    Then the service SHALL subscribe to the new variable without requiring a restart

  @AC-302.4
  Scenario: Subscriptions survive MSFS sim reset after airplane change
    Given the service has active SimConnect variable subscriptions
    When the user changes the aircraft in MSFS causing a sim reset
    Then the service SHALL re-establish all variable subscriptions automatically after the reset completes

  @AC-302.5
  Scenario: Variable access latency is less than 2ms from SimConnect notification
    Given a SimConnect variable subscription is active
    When SimConnect delivers a data update notification
    Then the service SHALL process and forward the updated value within 2 milliseconds

  @AC-302.6
  Scenario: Unsupported variable names produce a logged warning not an error
    Given a profile that references a SimConnect variable name not recognised by MSFS
    When the service attempts to subscribe to that variable
    Then the service SHALL log a warning message and continue running rather than returning an error
