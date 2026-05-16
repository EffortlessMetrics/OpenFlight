// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

use ed25519_dalek::{SigningKey, pkcs8::DecodePrivateKey};
use uselesskey::{Ed25519FactoryExt, Ed25519Spec, Factory, Seed};

/// Build a deterministic Ed25519 signing key without committing crypto fixtures.
pub fn deterministic_signing_key(seed_value: &str, label: &str) -> SigningKey {
    let seed = Seed::from_env_value(seed_value).expect("test seed must be valid");
    let factory = Factory::deterministic(seed);
    let keypair = factory.ed25519(label, Ed25519Spec::new());

    SigningKey::from_pkcs8_der(keypair.private_key_pkcs8_der().as_ref())
        .expect("uselesskey should emit valid Ed25519 PKCS#8")
}
