@REQ-239 @product
Feature: Two physical axes can be mixed to produce a virtual axis  @AC-239.1
  Scenario: Differential mix mode produces left minus right divided by two
    Given two physical axes mixed in differential mode
    When the left axis reads 0.8 and the right axis reads 0.4
    Then the virtual axis output SHALL be 0.2 which equals (0.8 - 0.4) / 2  @AC-239.2
  Scenario: Average mix mode produces left plus right divided by two
    Given two physical axes mixed in average mode
    When the left axis reads 0.6 and the right axis reads 0.4
    Then the virtual axis output SHALL be 0.5 which equals (0.6 + 0.4) / 2  @AC-239.3
  Scenario: Sum mix mode produces clamped sum of both axes
    Given two physical axes mixed in sum mode
    When the left axis reads 0.8 and the right axis reads 0.7
    Then the virtual axis output SHALL be clamped to 1.0 as sum 1.5 exceeds the limit  @AC-239.4
  Scenario: Virtual axis appears on bus as if it were a physical axis
    Given a virtual axis produced by mixing two physical axes
    When the bus snapshot is queried
    Then the virtual axis SHALL appear with its own identifier indistinguishable from physical axes  @AC-239.5
  Scenario: Virtual axis mixing configured per profile
    Given two profiles with different mix mode configurations for the same physical axes
    When each profile is active
    Then each profile SHALL produce the virtual axis output according to its own mix configuration  @AC-239.6
  Scenario: Disconnection of source axis freezes virtual axis at last value
    Given a virtual axis currently producing output from two physical sources
    When one of the source physical axes disconnects
    Then the virtual axis output SHALL freeze at its last computed value
