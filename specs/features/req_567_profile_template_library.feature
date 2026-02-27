Feature: Profile Template Library
  As a flight simulation enthusiast
  I want a library of profile templates for common aircraft types
  So that I can quickly start with a sensible profile without building one from scratch

  Background:
    Given the OpenFlight service is running
    And the template library is shipped with the service installation

  Scenario: Template library contains profiles for common aircraft types
    When the template library is inspected
    Then templates exist for at least general aviation, airliner, and combat aircraft categories

  Scenario: Templates are versioned and shipped with the service
    When the service is installed
    Then each template file contains a version field matching the service release version

  Scenario: flightctl profile templates lists available templates
    When the operator runs "flightctl profile templates"
    Then the CLI prints a table of available template names, categories, and versions

  Scenario: New profile can be created from a template via CLI
    When the operator runs "flightctl profile new --template general_aviation my_profile.toml"
    Then a new profile file "my_profile.toml" is created based on the general_aviation template
    And the CLI confirms the profile was created successfully
