Feature: Service Config Inheritance
  As a system operator
  I want service config files to support inheritance from a base config
  So that shared configuration can be maintained in one place

  Background:
    Given the OpenFlight service is available

  Scenario: Config files can declare extends: path to base config
    Given a child config file with an extends field pointing to a base config
    When the service loads the child config
    Then values from the base config are inherited

  Scenario: Inherited values can be overridden in child config
    Given a child config that overrides a value from the base config
    When the service loads the child config
    Then the child config value takes precedence over the inherited base value

  Scenario: Circular inheritance is detected and rejected
    Given two config files that extend each other circularly
    When the service attempts to load one of the configs
    Then a circular inheritance error is reported and loading is rejected

  Scenario: Effective config after inheritance is loggable via CLI
    Given a config with inheritance is active
    When the operator runs the CLI command to show effective config
    Then the fully resolved config including all inherited values is displayed
