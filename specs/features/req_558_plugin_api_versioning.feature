@REQ-558 @product
Feature: Plugin API Versioning — Plugin API should have explicit versioning

  @AC-558.1
  Scenario: Plugin manifest declares minimum API version required
    Given a plugin with a manifest file
    When the manifest is parsed
    Then it SHALL contain a minimum_api_version field

  @AC-558.2
  Scenario: Service rejects plugins requiring higher API version
    Given a plugin manifest declaring a minimum_api_version higher than the service API version
    When the service attempts to load the plugin
    Then the service SHALL reject the plugin and log a version incompatibility error

  @AC-558.3
  Scenario: Deprecated API features emit a warning when used
    Given a plugin that calls a deprecated API feature
    When the plugin is loaded and the deprecated feature is invoked
    Then the service SHALL emit a deprecation warning to the log

  @AC-558.4
  Scenario: API changelog is maintained in documentation
    Given the plugin API documentation
    When the API changelog is inspected
    Then it SHALL contain an entry for every API version describing changes from the previous version
