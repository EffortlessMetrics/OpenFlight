Feature: Axis Filter Chain Visualization
  As a flight simulation enthusiast
  I want the service to provide filter chain visualization data
  So that I can understand how my filter settings affect axis responsiveness

  Background:
    Given the OpenFlight service is running with a filter chain configured on an axis

  Scenario: FilterChainPreview RPC returns frequency response data
    When the FilterChainPreview gRPC RPC is called for an axis
    Then the response contains frequency response data for the configured filter chain

  Scenario: Preview shows gain at 10, 50, 100, 250 Hz
    When the FilterChainPreview RPC is called
    Then the response includes gain values at 10 Hz, 50 Hz, 100 Hz, and 250 Hz

  Scenario: Preview is valid for current filter configuration
    Given the filter chain configuration has been updated
    When the FilterChainPreview RPC is called after the update
    Then the returned frequency response reflects the new filter configuration

  Scenario: Preview generation does not require active input
    Given no physical input device is producing axis movement
    When the FilterChainPreview RPC is called
    Then the RPC returns a valid frequency response without requiring active input
