Feature: Compatibility Matrix Generation
  As a flight simulation enthusiast
  I want compatibility matrix generation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Compatibility matrix is auto-generated from device and simulator manifests
    Given the system is configured for compatibility matrix generation
    When the feature is exercised
    Then compatibility matrix is auto-generated from device and simulator manifests

  Scenario: Matrix includes supported OS versions, simulator versions, and device models
    Given the system is configured for compatibility matrix generation
    When the feature is exercised
    Then matrix includes supported OS versions, simulator versions, and device models

  Scenario: Generated matrix is published as part of release documentation
    Given the system is configured for compatibility matrix generation
    When the feature is exercised
    Then generated matrix is published as part of release documentation

  Scenario: Matrix validation runs against actual test results in CI pipeline
    Given the system is configured for compatibility matrix generation
    When the feature is exercised
    Then matrix validation runs against actual test results in CI pipeline