@REQ-355 @product
Feature: Profile Inheritance Resolution  @AC-355.1
  Scenario: Child profile merges parent defaults via inherits field
    Given a parent profile with id "base" defining axis curve settings
    When a child profile specifying "inherits: base" is compiled
    Then the child profile SHALL include the parent axis curve settings as defaults  @AC-355.2
  Scenario: Inheritance is resolved depth-first up to 5 levels deep
    Given a chain of 5 profiles where each inherits from the previous
    When the leaf profile is compiled
    Then the resolved profile SHALL include merged values from all 5 ancestors  @AC-355.3
  Scenario: Circular inheritance is detected and returns a compile error
    Given profile A inherits from profile B and profile B inherits from profile A
    When the system attempts to compile either profile
    Then compilation SHALL fail with a circular-inheritance error  @AC-355.4
  Scenario: Child values override parent values
    Given a parent profile defining deadzone as 0.05
    And a child profile defining deadzone as 0.10 and inheriting from the parent
    When the child profile is compiled
    Then the resolved deadzone SHALL be 0.10  @AC-355.5
  Scenario: Arrays in child profiles replace parent arrays entirely
    Given a parent profile with an axis list containing two entries
    And a child profile defining a single-entry axis list
    When the child profile is compiled
    Then the resolved axis list SHALL contain only the child's single entry  @AC-355.6
  Scenario: Resolved profile snapshot is cached and invalidated on source change
    Given a compiled child profile has been cached
    When the parent profile source file is modified
    Then the cached snapshot SHALL be invalidated and recompiled on next access
