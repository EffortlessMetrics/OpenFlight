Feature: Sim Plugin Deployment
  As a flight simulation enthusiast
  I want sim plugin deployment
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Installer auto-detects installed simulators and their plugin directories
    Given the system is configured for sim plugin deployment
    When the feature is exercised
    Then installer auto-detects installed simulators and their plugin directories

  Scenario: MSFS community folder receives WASM plugin during installation
    Given the system is configured for sim plugin deployment
    When the feature is exercised
    Then mSFS community folder receives WASM plugin during installation

  Scenario: X-Plane plugins directory receives native plugin during installation
    Given the system is configured for sim plugin deployment
    When the feature is exercised
    Then x-Plane plugins directory receives native plugin during installation

  Scenario: DCS scripts directory receives export script during installation
    Given the system is configured for sim plugin deployment
    When the feature is exercised
    Then dCS scripts directory receives export script during installation
