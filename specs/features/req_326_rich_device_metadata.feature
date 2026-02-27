@REQ-326 @product
Feature: Rich Device Metadata  @AC-326.1
  Scenario: Service queries device manufacturer string via USB descriptor
    Given a USB HID device is connected
    When the service enumerates the device
    Then the service SHALL query and store the manufacturer string from the USB descriptor  @AC-326.2
  Scenario: Product name and serial number are logged on connect
    Given a device with a USB product name and serial number
    When the device connects
    Then the service SHALL log the product name and serial number at info level  @AC-326.3
  Scenario: Metadata is included in device enumeration API response
    Given the service is running and devices are connected
    When a client calls the device enumeration RPC
    Then the response SHALL include manufacturer, product name, and serial number for each device  @AC-326.4
  Scenario: Metadata is included in diagnostic bundle
    Given a diagnostic bundle is generated via flightctl diag
    When the bundle is inspected
    Then the bundle SHALL contain the full USB metadata for all connected devices  @AC-326.5
  Scenario: Devices without USB descriptor strings use VID/PID as identifier
    Given a device that returns empty USB descriptor strings
    When the service identifies the device
    Then the service SHALL fall back to using the VID/PID string as the device identifier  @AC-326.6
  Scenario: Metadata is cached per-session
    Given a device that has already been enumerated in the current session
    When the service needs to display device metadata again
    Then the service SHALL return the cached metadata without issuing another USB descriptor query
