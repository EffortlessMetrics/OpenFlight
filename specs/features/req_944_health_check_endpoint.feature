Feature: Health Check Endpoint
  As a flight simulation enthusiast
  I want health check endpoint
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: HTTP health check endpoint returns service status for monitoring
    Given the system is configured for health check endpoint
    When the feature is exercised
    Then hTTP health check endpoint returns service status for monitoring

  Scenario: Health response includes component-level status for each subsystem
    Given the system is configured for health check endpoint
    When the feature is exercised
    Then health response includes component-level status for each subsystem

  Scenario: Health check distinguishes between healthy, degraded, and unhealthy states
    Given the system is configured for health check endpoint
    When the feature is exercised
    Then health check distinguishes between healthy, degraded, and unhealthy states

  Scenario: Health endpoint responds within 100ms under normal operating conditions
    Given the system is configured for health check endpoint
    When the feature is exercised
    Then health endpoint responds within 100ms under normal operating conditions
