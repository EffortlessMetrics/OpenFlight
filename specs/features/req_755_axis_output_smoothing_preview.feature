Feature: Axis Output Smoothing Preview
  As a flight simulation enthusiast
  I want axis output smoothing preview
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Preview smoothing effect
    Given the system is configured for axis output smoothing preview
    When the feature is exercised
    Then cli previews the effect of smoothing settings on sample data

  Scenario: Before and after comparison
    Given the system is configured for axis output smoothing preview
    When the feature is exercised
    Then preview displays before and after comparison

  Scenario: All smoothing algorithms supported
    Given the system is configured for axis output smoothing preview
    When the feature is exercised
    Then preview supports all smoothing algorithm types

  Scenario: Exportable preview output
    Given the system is configured for axis output smoothing preview
    When the feature is exercised
    Then preview output is exportable to file
