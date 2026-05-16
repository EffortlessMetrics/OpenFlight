// SPDX-License-Identifier: MIT OR Apache-2.0

//! Documentation front matter repair and normalization.
//!
//! This is the Rust replacement for the former Python fixer. It preserves each
//! Markdown body while normalizing the metadata fields consumed by the docs
//! cross-reference tooling.

use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};
use std::path::Path;
use walkdir::WalkDir;

const DOCS_DIR: &str = "docs";

pub fn run_fix_front_matter() -> Result<()> {
    for entry in WalkDir::new(DOCS_DIR)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
    {
        let path = entry.into_path();
        println!("Processing {}", path.display());
        fix_file(&path)?;
    }

    Ok(())
}

fn fix_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let (mut front_matter, body) = split_front_matter(&content)
        .with_context(|| format!("failed to parse front matter in {}", path.display()))?;

    normalize_legacy_field(&mut front_matter, "category", "kind", map_kind);
    normalize_legacy_field(&mut front_matter, "group", "area", map_area);

    if !contains_key(&front_matter, "kind") {
        set_value(
            &mut front_matter,
            "kind",
            Value::String(default_kind(path).to_string()),
        );
    }
    if !contains_key(&front_matter, "area") {
        set_value(
            &mut front_matter,
            "area",
            Value::String(default_area(&content).to_string()),
        );
    }
    if !contains_key(&front_matter, "status") {
        set_value(
            &mut front_matter,
            "status",
            Value::String("draft".to_string()),
        );
    }

    let mut links = take_mapping(&mut front_matter, "links").unwrap_or_default();
    move_legacy_link(&mut front_matter, &mut links, "requirements");
    move_legacy_link(&mut front_matter, &mut links, "tasks");
    move_legacy_link(&mut front_matter, &mut links, "adrs");
    for key in ["requirements", "tasks", "adrs"] {
        ensure_sequence(&mut links, key);
    }

    let kind = get_value(&front_matter, "kind")
        .and_then(Value::as_str)
        .unwrap_or("concept")
        .to_string();
    set_value(
        &mut front_matter,
        "doc_id",
        Value::String(format!("DOC-{}-{}", slugify(&kind), doc_slug(path)?)),
    );

    let mut ordered = Mapping::new();
    for key in ["doc_id", "kind", "area", "status"] {
        if let Some(value) = front_matter.remove(key) {
            ordered.insert(Value::String(key.to_string()), value);
        }
    }
    ordered.insert(Value::String("links".to_string()), Value::Mapping(links));

    let yaml = serde_yaml::to_string(&ordered).context("failed to serialize front matter")?;
    let new_content = format!("---\n{}---\n{}", yaml, body);
    std::fs::write(path, new_content).with_context(|| format!("failed to write {}", path.display()))
}

fn split_front_matter(content: &str) -> Result<(Mapping, &str)> {
    if let Some(stripped) = content.strip_prefix("---\n")
        && let Some(end) = stripped.find("\n---\n")
    {
        let yaml = &stripped[..end];
        let body = &stripped[end + "\n---\n".len()..];
        let map = serde_yaml::from_str::<Mapping>(yaml).context("invalid YAML front matter")?;
        return Ok((map, body));
    }

    Ok((Mapping::new(), content))
}

fn normalize_legacy_field(
    front_matter: &mut Mapping,
    old_key: &str,
    new_key: &str,
    mapper: fn(&str) -> &str,
) {
    if let Some(value) = remove_value(front_matter, old_key)
        && let Some(raw) = value.as_str()
    {
        front_matter.insert(
            Value::String(new_key.to_string()),
            Value::String(mapper(raw).to_string()),
        );
    }
}

fn move_legacy_link(front_matter: &mut Mapping, links: &mut Mapping, key: &str) {
    if let Some(value) = remove_value(front_matter, key) {
        links.insert(Value::String(key.to_string()), value);
    }
}

fn ensure_sequence(links: &mut Mapping, key: &str) {
    let key_value = Value::String(key.to_string());
    match links.remove(&key_value) {
        Some(Value::Sequence(values)) => {
            links.insert(key_value, Value::Sequence(values));
        }
        Some(Value::Null) | None => {
            links.insert(key_value, Value::Sequence(Vec::new()));
        }
        Some(value) => {
            links.insert(key_value, Value::Sequence(vec![value]));
        }
    }
}

fn take_mapping(front_matter: &mut Mapping, key: &str) -> Option<Mapping> {
    match remove_value(front_matter, key) {
        Some(Value::Mapping(mapping)) => Some(mapping),
        _ => None,
    }
}

fn remove_value(mapping: &mut Mapping, key: &str) -> Option<Value> {
    mapping.remove(Value::String(key.to_string()))
}

fn contains_key(mapping: &Mapping, key: &str) -> bool {
    mapping.contains_key(Value::String(key.to_string()))
}

fn get_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_string()))
}

fn set_value(mapping: &mut Mapping, key: &str, value: Value) {
    mapping.insert(Value::String(key.to_string()), value);
}

fn default_kind(path: &Path) -> &'static str {
    let parts: Vec<_> = path.components().collect();
    let has = |needle: &str| {
        parts
            .iter()
            .any(|part| part.as_os_str().to_string_lossy() == needle)
    };

    if has("explanation") {
        if has("adr") {
            return "adr";
        }
        return "explanation";
    }
    if has("how-to") {
        return "how-to";
    }
    if has("reference") {
        return "reference";
    }
    if has("tutorials") {
        return "tutorial";
    }
    if has("design") {
        return "design";
    }
    if has("dev") {
        return "explanation";
    }
    "concept"
}

fn default_area(content: &str) -> &'static str {
    let content = content.to_lowercase();
    for area in [
        "flight-core",
        "flight-virtual",
        "flight-hid",
        "flight-ipc",
        "flight-scheduler",
        "flight-ffb",
        "flight-panels",
        "infra",
        "ci",
        "simulation",
        "integration",
        "ksp",
        "profile",
    ] {
        if content.contains(area) {
            return area;
        }
    }
    "flight-core"
}

fn map_kind(value: &str) -> &str {
    match value {
        "explanation" => "explanation",
        "how-to" => "how-to",
        "reference" => "reference",
        "tutorial" => "tutorial",
        "design" => "design",
        "concept" => "concept",
        "requirements" => "requirements",
        "adr" => "adr",
        _ => "concept",
    }
}

fn map_area(value: &str) -> &str {
    match value {
        "flight-core" => "flight-core",
        "flight-virtual" => "flight-virtual",
        "flight-hid" => "flight-hid",
        "flight-ipc" => "flight-ipc",
        "flight-scheduler" => "flight-scheduler",
        "flight-ffb" => "flight-ffb",
        "flight-panels" => "flight-panels",
        "infra" | "infrastructure" => "infra",
        "ci" => "ci",
        "simulation" => "simulation",
        "integration" => "integration",
        "ksp" => "ksp",
        "profile" => "profile",
        _ => "flight-core",
    }
}

fn doc_slug(path: &Path) -> Result<String> {
    let mut slug_parts = Vec::new();
    if let Ok(relative_parent) = path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .strip_prefix(DOCS_DIR)
    {
        for part in relative_parent.components() {
            slug_parts.push(part.as_os_str().to_string_lossy().into_owned());
        }
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .with_context(|| format!("failed to get file stem for {}", path.display()))?;
    slug_parts.push(stem);
    Ok(slugify(&slug_parts.join("-")))
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = true;
    for ch in value.chars().flat_map(char::to_uppercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}
