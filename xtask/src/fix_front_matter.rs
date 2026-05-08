// SPDX-License-Identifier: MIT OR Apache-2.0

//! Documentation front matter fixer.
//!
//! This module is the Rust replacement for `scripts/fix_front_matter.py`. It
//! rewrites docs front matter into the canonical schema used by validation.

use anyhow::{Context, Result};
use regex::Regex;
use serde_yaml::{Mapping, Value};
use std::path::Path;
use walkdir::WalkDir;

const DOCS_DIR: &str = "docs";

/// Rewrite every docs/**/*.md front matter block into the canonical shape.
pub fn run_fix_front_matter() -> Result<()> {
    let mut files = Vec::new();
    for entry in WalkDir::new(DOCS_DIR)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(path.to_path_buf());
        }
    }
    files.sort();

    for md_file in files {
        println!("Processing {}", md_file.display());
        fix_file(&md_file)?;
    }

    Ok(())
}

fn fix_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let (mut front_matter, body) = split_front_matter(&content)
        .with_context(|| format!("failed to parse YAML in {}", path.display()))?;

    normalize_front_matter(path, &content, &mut front_matter)?;

    let new_content = format!(
        "---\n{}---\n{}",
        serde_yaml::to_string(&front_matter).context("failed to serialize front matter")?,
        body
    );
    std::fs::write(path, new_content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn split_front_matter(content: &str) -> Result<(Mapping, &str)> {
    let Some(after_open) = content.strip_prefix("---\n") else {
        return Ok((Mapping::new(), content));
    };
    let Some(close_offset) = after_open.find("\n---\n") else {
        return Ok((Mapping::new(), content));
    };

    let yaml = &after_open[..close_offset];
    let body = &after_open[close_offset + "\n---\n".len()..];
    let parsed = serde_yaml::from_str::<Option<Mapping>>(yaml)?.unwrap_or_default();
    Ok((parsed, body))
}

fn normalize_front_matter(path: &Path, content: &str, front_matter: &mut Mapping) -> Result<()> {
    rename_mapped_field(front_matter, "category", "kind", map_kind);
    rename_mapped_field(front_matter, "group", "area", map_area);

    if !front_matter.contains_key("kind") {
        insert_str(front_matter, "kind", default_kind(path));
    }
    if !front_matter.contains_key("area") {
        insert_str(front_matter, "area", default_area(content));
    }
    if !front_matter.contains_key("status") {
        insert_str(front_matter, "status", "draft");
    }

    let mut links = remove_mapping(front_matter, "links").unwrap_or_default();
    move_legacy_link(front_matter, &mut links, "requirements");
    move_legacy_link(front_matter, &mut links, "tasks");
    move_legacy_link(front_matter, &mut links, "adrs");
    for key in ["requirements", "tasks", "adrs"] {
        ensure_array(&mut links, key);
    }

    let kind = get_string(front_matter, "kind").context("kind must be a string")?;
    insert_str(
        front_matter,
        "doc_id",
        &format!("DOC-{}-{}", kind.to_uppercase(), doc_slug(path)?),
    );

    let mut ordered = Mapping::new();
    for key in ["doc_id", "kind", "area", "status"] {
        if let Some(value) = front_matter.remove(Value::String(key.to_owned())) {
            ordered.insert(Value::String(key.to_owned()), value);
        }
    }
    ordered.insert(Value::String("links".to_owned()), Value::Mapping(links));
    *front_matter = ordered;
    Ok(())
}

fn rename_mapped_field(front_matter: &mut Mapping, from: &str, to: &str, mapper: fn(&str) -> &str) {
    if let Some(value) = front_matter.remove(Value::String(from.to_owned())) {
        let mapped = value.as_str().map(mapper).unwrap_or("concept");
        insert_str(front_matter, to, mapped);
    }
}

fn remove_mapping(front_matter: &mut Mapping, key: &str) -> Option<Mapping> {
    match front_matter.remove(Value::String(key.to_owned())) {
        Some(Value::Mapping(mapping)) => Some(mapping),
        Some(_) | None => None,
    }
}

fn move_legacy_link(front_matter: &mut Mapping, links: &mut Mapping, key: &str) {
    if let Some(value) = front_matter.remove(Value::String(key.to_owned())) {
        links.insert(Value::String(key.to_owned()), value);
    }
}

fn ensure_array(mapping: &mut Mapping, key: &str) {
    let key_value = Value::String(key.to_owned());
    match mapping.remove(&key_value) {
        Some(Value::Sequence(sequence)) => {
            mapping.insert(key_value, Value::Sequence(sequence));
        }
        Some(value) => {
            mapping.insert(key_value, Value::Sequence(vec![value]));
        }
        None => {
            mapping.insert(key_value, Value::Sequence(Vec::new()));
        }
    }
}

fn insert_str(mapping: &mut Mapping, key: &str, value: &str) {
    mapping.insert(
        Value::String(key.to_owned()),
        Value::String(value.to_owned()),
    );
}

fn get_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
}

fn default_kind(path: &Path) -> &'static str {
    let parts: Vec<_> = path
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect();
    if parts.contains(&"explanation") {
        if parts.contains(&"adr") {
            return "adr";
        }
        return "explanation";
    }
    if parts.contains(&"how-to") {
        return "how-to";
    }
    if parts.contains(&"reference") {
        return "reference";
    }
    if parts.contains(&"tutorials") {
        return "tutorial";
    }
    if parts.contains(&"design") {
        return "design";
    }
    if parts.contains(&"dev") {
        return "explanation";
    }
    "concept"
}

fn default_area(content: &str) -> &'static str {
    let content_lower = content.to_lowercase();
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
        if content_lower.contains(area) {
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
    let mut name_slug = slugify(
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default(),
    );
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let parent_parts = parent
        .strip_prefix(DOCS_DIR)
        .unwrap_or(parent)
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>();
    if !parent_parts.is_empty() {
        name_slug = format!("{}-{}", slugify(&parent_parts.join("-")), name_slug);
    }
    Ok(name_slug)
}

fn slugify(name: &str) -> String {
    let re = Regex::new(r"[^A-Z0-9]+").expect("slug regex must compile");
    re.replace_all(&name.to_uppercase(), "-")
        .trim_matches('-')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_includes_docs_parent() {
        assert_eq!(
            doc_slug(&std::path::PathBuf::from("docs/how-to/setup-linux.md")).unwrap(),
            "HOW-TO-SETUP-LINUX"
        );
    }

    #[test]
    fn defaults_kind_from_path() {
        assert_eq!(
            default_kind(Path::new("docs/explanation/adr/adr-001.md")),
            "adr"
        );
        assert_eq!(
            default_kind(Path::new("docs/tutorials/install.md")),
            "tutorial"
        );
    }
}
