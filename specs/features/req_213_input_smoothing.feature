@REQ-213 @product
Feature: Input smoothing filters reduce jitter without adding perceptible latency  @AC-213.1
  Scenario: EMA filter configurable per axis with alpha in range 0.0 to 1.0
    Given an axis with an EMA smoothing filter configured
    When an alpha value between 0.0 and 1.0 inclusive is set for the axis
    Then the filter SHALL apply exponential moving average smoothing using the configured alpha  @AC-213.2
  Scenario: Alpha of 1.0 produces passthrough and alpha of 0.1 is heavy smoothing
    Given an axis with EMA filter enabled
    When alpha is set to 1.0
    Then the output SHALL equal the raw input without any smoothing  @AC-213.3
  Scenario: EMA filter state is preserved across ticks
    Given an axis with EMA filter active and a prior tick producing a filtered value
    When the next tick arrives with new raw input
    Then the filter SHALL use the previous tick output as the prior state for the EMA computation  @AC-213.4
  Scenario: Filter latency measurable as 1 divided by 1 minus alpha ticks at 250Hz
    Given an axis EMA filter with alpha set to 0.9
    When a step input is applied at 250Hz
    Then the time-to-settle SHALL correspond to approximately 1 divided by quantity 1 minus alpha ticks  @AC-213.5
  Scenario: Smoothing enabled or disabled via profile without service restart
    Given a running service with an active profile
    When the profile is updated to toggle EMA smoothing on an axis
    Then the change SHALL take effect at the next tick boundary without restarting the service  @AC-213.6
  Scenario: Combined deadzone plus smoothing plus rate-limit chain produces stable output
    Given an axis configured with deadzone, EMA smoothing, and rate-limit all enabled
    When noisy input is provided continuously
    Then the output SHALL be stable and free of rapid oscillation throughout the processing chain
