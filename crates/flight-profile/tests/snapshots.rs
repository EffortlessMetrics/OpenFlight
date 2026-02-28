// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for built-in profile templates and profile diff output.
//!
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_profile::profile_compare::{compare_profiles, flatten_profile};
use flight_profile::templates::Template;

// ── Built-in template snapshots (JSON) ───────────────────────────────────────

#[test]
fn snapshot_template_default_flight_json() {
    let profile = Template::default_flight();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_default_flight", profile);
    });
}

#[test]
fn snapshot_template_helicopter_json() {
    let profile = Template::helicopter();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_helicopter", profile);
    });
}

#[test]
fn snapshot_template_space_sim_json() {
    let profile = Template::space_sim();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_space_sim", profile);
    });
}

#[test]
fn snapshot_template_airliner_json() {
    let profile = Template::airliner();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_airliner", profile);
    });
}

#[test]
fn snapshot_template_warbird_json() {
    let profile = Template::warbird();
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("template_warbird", profile);
    });
}

// ── Profile diff output snapshots ────────────────────────────────────────────

#[test]
fn snapshot_profile_diff_text_output() {
    let base = Template::default_flight();
    let modified = Template::warbird();

    let base_json = serde_json::to_value(&base).unwrap();
    let mod_json = serde_json::to_value(&modified).unwrap();

    let left = flatten_profile(&base_json, "");
    let right = flatten_profile(&mod_json, "");

    let diff = compare_profiles(&left, &right, "default_flight", "warbird");
    insta::assert_snapshot!("profile_diff_default_vs_warbird", diff.to_text());
}

#[test]
fn snapshot_profile_diff_axes_filtered() {
    let base = Template::default_flight();
    let modified = Template::airliner();

    let base_json = serde_json::to_value(&base).unwrap();
    let mod_json = serde_json::to_value(&modified).unwrap();

    let left = flatten_profile(&base_json, "");
    let right = flatten_profile(&mod_json, "");

    let diff = compare_profiles(&left, &right, "default_flight", "airliner");
    let axes_only = diff.filter_by_prefix("axes");
    insta::assert_snapshot!(
        "profile_diff_axes_only_default_vs_airliner",
        axes_only.to_text()
    );
}
