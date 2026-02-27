@REQ-317 @product
Feature: Secure Update Verification  @AC-317.1
  Scenario: Update package is signed with a developer key
    Given an update package prepared for distribution
    When the package is inspected
    Then the update package SHALL contain a cryptographic signature produced by the developer signing key  @AC-317.2
  Scenario: Signature verification uses Ed25519 and not RSA
    Given the update verification implementation
    When the signature algorithm is inspected
    Then the service SHALL use Ed25519 or an equivalent modern algorithm and SHALL NOT use RSA for signature verification  @AC-317.3
  Scenario: Verification failure rejects the update and logs the error
    Given an update package with an invalid or tampered signature
    When the service attempts to apply the update
    Then the service SHALL reject the update, not apply any changes, and log the verification failure with details  @AC-317.4
  Scenario: Public key is embedded in the service binary (pinned)
    Given the compiled service binary
    When the update verification public key source is inspected
    Then the service SHALL use a public key that is statically embedded (pinned) in the binary at compile time  @AC-317.5
  Scenario: Update manifest includes expected hash and version
    Given a valid signed update package
    When the update manifest is parsed
    Then the manifest SHALL contain the expected cryptographic hash and version string of the update payload  @AC-317.6
  Scenario: CLI shows verification status before applying update
    Given the CLI is invoked to apply an update
    When update verification completes (pass or fail)
    Then the CLI SHALL display the verification status to the user before any update is applied
