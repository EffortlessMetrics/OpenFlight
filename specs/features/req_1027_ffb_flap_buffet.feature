@REQ-1027
Feature: FFB Flap Buffet
  @AC-1027.1
  Scenario: Flap deployment produces aerodynamic buffet feedback
    Given the system is configured for REQ-1027
    When the feature condition is met
    Then flap deployment produces aerodynamic buffet feedback

  @AC-1027.2
  Scenario: Buffet characteristics vary by flap position setting
    Given the system is configured for REQ-1027
    When the feature condition is met
    Then buffet characteristics vary by flap position setting

  @AC-1027.3
  Scenario: Flap buffet interacts correctly with airspeed scaling
    Given the system is configured for REQ-1027
    When the feature condition is met
    Then flap buffet interacts correctly with airspeed scaling

  @AC-1027.4
  Scenario: Effect parameters are configurable per aircraft and flap detent
    Given the system is configured for REQ-1027
    When the feature condition is met
    Then effect parameters are configurable per aircraft and flap detent
