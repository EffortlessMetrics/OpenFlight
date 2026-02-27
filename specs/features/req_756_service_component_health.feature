Feature: Service Component Health
  As a flight simulation enthusiast
  I want service component health
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Track per-component health
    Given the system is configured for service component health
    When the feature is exercised
    Then service tracks health status for each registered component

  Scenario: Health states include healthy degraded failed
    Given the system is configured for service component health
    When the feature is exercised
    Then health status includes healthy, degraded, and failed states

  Scenario: Health queryable via gRPC
    Given the system is configured for service component health
    When the feature is exercised
    Then component health is queryable via grpc api

  Scenario: Health transitions emit events
    Given the system is configured for service component health
    When the feature is exercised
    Then health transitions emit events on the bus
