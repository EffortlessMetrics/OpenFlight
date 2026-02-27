@REQ-510 @product
Feature: Release Artifact Signing — Binaries Signed and Verifiable  @AC-510.1
  Scenario: Release binaries are signed with project key in CI pipeline
    Given a release build completes successfully in CI
    When the signing step runs
    Then each release binary SHALL have a corresponding detached signature file  @AC-510.2
  Scenario: Signature files are published alongside binaries
    Given a release has been published to the distribution location
    When the release assets are listed
    Then a .sig or equivalent signature file SHALL accompany each binary artifact  @AC-510.3
  Scenario: flightctl verify command checks artifact signatures
    Given a downloaded release binary and its signature file
    When `flightctl verify <binary>` is executed
    Then the command SHALL report verified if the signature is valid and fail if tampered  @AC-510.4
  Scenario: Update system verifies signatures before applying updates
    Given the auto-updater has downloaded a new release package
    When the update is about to be applied
    Then the updater SHALL verify the package signature and abort if verification fails
