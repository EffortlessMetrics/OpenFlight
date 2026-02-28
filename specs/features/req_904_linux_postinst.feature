Feature: Linux Postinst Script
  As a flight simulation enthusiast
  I want linux postinst script
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Postinst script adds user to input group for HID access
    Given the system is configured for linux postinst script
    When the feature is exercised
    Then postinst script adds user to input group for HID access

  Scenario: Postinst triggers udev rule reload without requiring reboot
    Given the system is configured for linux postinst script
    When the feature is exercised
    Then postinst triggers udev rule reload without requiring reboot

  Scenario: Postinst enables systemd user unit when auto-start is configured
    Given the system is configured for linux postinst script
    When the feature is exercised
    Then postinst enables systemd user unit when auto-start is configured

  Scenario: Postinst validates installation integrity and reports errors
    Given the system is configured for linux postinst script
    When the feature is exercised
    Then postinst validates installation integrity and reports errors
