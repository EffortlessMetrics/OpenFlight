Feature: Linux Deb Package
  As a flight simulation enthusiast
  I want linux deb package
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Deb package installs binaries to standard FHS paths
    Given the system is configured for linux deb package
    When the feature is exercised
    Then deb package installs binaries to standard FHS paths

  Scenario: Package includes udev rules for supported HID devices
    Given the system is configured for linux deb package
    When the feature is exercised
    Then package includes udev rules for supported HID devices

  Scenario: Package includes systemd user unit file for auto-start
    Given the system is configured for linux deb package
    When the feature is exercised
    Then package includes systemd user unit file for auto-start

  Scenario: Package declares correct dependencies for libudev and libusb
    Given the system is configured for linux deb package
    When the feature is exercised
    Then package declares correct dependencies for libudev and libusb
