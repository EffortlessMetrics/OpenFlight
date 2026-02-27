Feature: Profile Auto-Create on First Run
  As a flight simulation enthusiast
  I want the service to create a default profile on first run
  So that I can start using OpenFlight immediately without manual setup

  Background:
    Given the OpenFlight service is installed

  Scenario: Service detects first-run via absence of profile directory
    Given the profile directory does not exist
    When the service starts
    Then it detects the first-run condition from the missing profile directory

  Scenario: Default profile is created from built-in template
    Given first-run is detected
    When the service initialises
    Then a default profile is created from the built-in profile template

  Scenario: Profile creation is logged with explanation
    Given first-run is detected
    When the default profile is created
    Then the creation event is logged with an explanation for the user

  Scenario: First-run detection is non-destructive on subsequent runs
    Given a profile directory already exists
    When the service starts
    Then first-run detection does not modify or replace existing profiles
