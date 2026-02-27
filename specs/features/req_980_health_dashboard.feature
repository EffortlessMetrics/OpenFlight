Feature: Health Dashboard
  As a flight simulation enthusiast
  I want health dashboard
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Web-based health overview displays system status in real time
    Given the system is configured for health dashboard
    When the feature is exercised
    Then web-based health overview displays system status in real time

  Scenario: Dashboard shows component health, device status, and sim connections
    Given the system is configured for health dashboard
    When the feature is exercised
    Then dashboard shows component health, device status, and sim connections

  Scenario: Historical health data is charted with configurable time window
    Given the system is configured for health dashboard
    When the feature is exercised
    Then historical health data is charted with configurable time window

  Scenario: Dashboard is accessible on configurable local port without authentication
    Given the system is configured for health dashboard
    When the feature is exercised
    Then dashboard is accessible on configurable local port without authentication