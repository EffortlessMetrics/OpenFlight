Feature: Axis Sample Interpolation
  As a flight simulation enthusiast
  I want the axis engine to support sample-rate interpolation
  So that devices with mismatched sample rates integrate seamlessly

  Background:
    Given the OpenFlight service is running

  Scenario: Upsampling fills intermediate samples using linear interpolation
    Given a device producing samples at a rate lower than the axis engine rate
    When the axis engine processes the samples
    Then intermediate samples are filled using linear interpolation

  Scenario: Downsampling uses decimation with anti-aliasing
    Given a device producing samples at a rate higher than the axis engine rate
    When the axis engine processes the samples
    Then downsampling is performed using decimation with anti-aliasing

  Scenario: Interpolation is activated automatically for mismatched rates
    Given a device whose sample rate differs from the axis engine rate
    When the device is connected
    Then the axis engine automatically activates interpolation for that device

  Scenario: Resampler adds at most one tick of latency
    Given sample-rate interpolation is active
    When axis samples are measured end-to-end
    Then the resampler contributes at most one axis tick of additional latency
