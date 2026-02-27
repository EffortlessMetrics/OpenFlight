Feature: MSFS PMDG Integration
  As a flight simulation enthusiast
  I want the service to support PMDG aircraft via custom SimConnect events
  So that I can bind controls to PMDG-specific systems in my profile

  Background:
    Given the OpenFlight service is running with the pmdg feature flag enabled

  Scenario: PMDG SDK data format is documented in integration guide
    When the integration guide is consulted
    Then it contains documentation of the PMDG SDK data format used by the adapter

  Scenario: PMDG-specific SimConnect events are bindable in profile
    Given a profile targeting a PMDG aircraft
    When the profile is authored
    Then PMDG-specific SimConnect events are available as bindable actions

  Scenario: PMDG adapter is gated behind pmdg feature flag
    Given the service is built without the pmdg feature flag
    When a profile references PMDG-specific events
    Then the PMDG adapter is not loaded and a clear error is reported

  Scenario: PMDG device support is listed in compatibility matrix
    When the compatibility matrix is inspected
    Then supported PMDG aircraft models are listed with their integration status
