@REQ-224 @infra
Feature: OpenFlight enforces platform security model for device and config access  @AC-224.1
  Scenario: Service runs without administrator or root privileges on Windows and Linux
    Given OpenFlight installed on Windows and Linux as a standard user
    When flightd is started without elevated privileges
    Then the service SHALL start and operate fully without requiring administrator or root on either platform  @AC-224.2
  Scenario: HID access controlled via udev rules on Linux and WinUSB on Windows
    Given udev rules deployed on Linux or WinUSB driver configured on Windows
    When a non-root user starts flightd
    Then HID device access SHALL be granted via the platform access control mechanism without elevation  @AC-224.3
  Scenario: Config files stored in user-scoped directory
    Given the service is running as a standard user account
    When the service reads or writes any configuration file
    Then all config file paths SHALL be within the user-scoped directory and not in system-wide writable paths  @AC-224.4
  Scenario: IPC channel authenticated via local socket filesystem permissions
    Given the IPC server is bound to a local socket with filesystem permissions set
    When a client process attempts to connect to the IPC channel
    Then the connection SHALL succeed only if the client process has filesystem-level permission to the socket  @AC-224.5
  Scenario: Plugin WASM sandbox cannot access filesystem or network
    Given a WASM plugin loaded and executing inside the plugin sandbox
    When the plugin attempts to open a file or initiate a network connection
    Then the attempt SHALL be denied by the sandbox and an error SHALL be returned to the plugin  @AC-224.6
  Scenario: Security model documented in SECURITY.md
    Given the OpenFlight repository root
    When SECURITY.md is inspected
    Then it SHALL document the privilege model, HID access method, IPC authentication, and plugin sandbox constraints
