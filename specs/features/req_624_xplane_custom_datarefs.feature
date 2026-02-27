Feature: X-Plane Custom DataRef Discovery
  As a flight simulation enthusiast
  I want the X-Plane adapter to discover custom DataRefs from installed plugins
  So that I can subscribe to aircraft-specific variables

  Background:
    Given the OpenFlight service is running
    And the X-Plane adapter is connected

  Scenario: Adapter queries DataRef Tool plugin for custom DataRef list
    Given the DataRef Tool plugin is installed in X-Plane
    When the X-Plane adapter connects
    Then it queries the DataRef Tool plugin for the list of custom DataRefs

  Scenario: Discovered DataRefs are available for subscription
    Given the X-Plane adapter has discovered custom DataRefs
    When a subscription is requested for a discovered custom DataRef
    Then the adapter subscribes and delivers values for that DataRef

  Scenario: Discovery runs once on connection and on explicit refresh
    Given the X-Plane adapter is connected
    When an explicit DataRef refresh is requested
    Then the adapter re-queries the DataRef Tool plugin and updates the discovered list

  Scenario: Custom DataRefs are documented in X-Plane adapter guide
    When the X-Plane adapter documentation is inspected
    Then the custom DataRef discovery mechanism is documented
