Feature: Prepar3D SimConnect Support
  As a flight simulation enthusiast
  I want to use OpenFlight with Prepar3D
  So that I can manage my flight controls in P3D as well as MSFS

  Background:
    Given the OpenFlight service is running

  Scenario: P3D SimConnect adapter uses same interface as MSFS adapter
    When the P3D SimConnect adapter is loaded
    Then it implements the same adapter interface as the MSFS SimConnect adapter

  Scenario: P3D-specific SimConnect features are gated behind p3d feature flag
    Given the service is compiled without the p3d feature flag
    When a P3D-specific feature is accessed
    Then it is not available

  Scenario: P3D version detection distinguishes v4 and v5
    When Prepar3D v4 is running
    Then the adapter detects and reports version 4
    When Prepar3D v5 is running
    Then the adapter detects and reports version 5

  Scenario: P3D is listed in compatibility matrix
    When the compatibility matrix is inspected
    Then Prepar3D is listed as a supported simulator
