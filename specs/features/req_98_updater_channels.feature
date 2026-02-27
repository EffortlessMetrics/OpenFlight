@REQ-98 @product
Feature: Software Update Channel Selection and Cryptographic Manifest Verification

  Background:
    Given the flight-updater crate with ChannelManager, SignatureVerifier, and UpdateSignature

  @AC-98.1
  Scenario: Three update channels exist — stable, beta, and canary
    Given the ChannelManager is created with default configuration
    When the available channels are queried
    Then a Stable channel SHALL exist
    And a Beta channel SHALL exist
    And a Canary channel SHALL exist

  @AC-98.1
  Scenario: Stable is the default channel when no channel is explicitly configured
    Given a ChannelConfig constructed with Default::default()
    When the channel field is inspected
    Then the channel SHALL equal Channel::Stable

  @AC-98.2
  Scenario: Channel can be switched to beta by name
    Given a ChannelManager initialised on the stable channel
    When set_channel is called with Channel::Beta
    Then current_channel() SHALL return Channel::Beta

  @AC-98.2
  Scenario: Parsing an unknown channel name returns ChannelNotFound error
    Given Channel::from_str is called with "nightly"
    When the result is inspected
    Then the result SHALL be Err(UpdateError::ChannelNotFound)

  @AC-98.3
  Scenario: Valid Ed25519-signed update manifest passes cryptographic verification
    Given a SignatureVerifier initialised with a known Ed25519 key pair
    When a manifest is signed with the private key and verified with the public key
    Then the verification result SHALL be Ok(true)

  @AC-98.3
  Scenario: Tampered manifest content fails verification
    Given a SignatureVerifier and a manifest signed with a valid key
    When a single byte of the manifest content is altered before verification
    Then the verification result SHALL indicate failure

  @AC-98.4
  Scenario: Unsupported signature algorithm returns an explicit error
    Given a SignatureVerifier and an UpdateSignature whose algorithm field is "RSA-2048"
    When verify is called
    Then the result SHALL be Err with an unsupported-algorithm description

  @AC-98.4
  Scenario: Invalid hex-encoded signature bytes return an explicit error
    Given a SignatureVerifier and an UpdateSignature whose signature field contains non-hex characters
    When verify is called
    Then the result SHALL be Err with an invalid-signature description
