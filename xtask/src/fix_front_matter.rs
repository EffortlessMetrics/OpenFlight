// SPDX-License-Identifier: MIT OR Apache-2.0

//! Repair and normalize documentation front matter.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

const DOCS_DIR: &str = "docs";

#[derive(Serialize)]
struct NormalizedFrontMatter {
    doc_id: String,
    kind: String,
    area: String,
    status: String,
    links: NormalizedLinks,
}

#[derive(Serialize)]
struct NormalizedLinks {
    requirements: Vec<String>,
    tasks: Vec<String>,
    adrs: Vec<String>,
}

pub fn run_fix_front_matter() -> Result<()> {
    for entry in WalkDir::new(DOCS_DIR)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
    {
        println!("Processing {}", entry.path().display());
        fix_file(entry.path())?;
    }

    Ok(())
}

fn fix_file(path: &Path) -> Result<()> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (mut front_matter, body) = split_front_matter(&content)
        .with_context(|| format!("failed to parse front matter in {}", path.display()))?;

    if let Some(category) = remove_string(&mut front_matter, "category") {
        front_matter.insert(string_value("kind"), string_value(map_kind(&category)));
    }
    if let Some(group) = remove_string(&mut front_matter, "group") {
        front_matter.insert(string_value("area"), string_value(map_area(&group)));
    }

    let kind = get_string(&front_matter, "kind")
        .map(|value| map_kind(&value).to_string())
        .unwrap_or_else(|| get_default_kind(path).to_string());
    let area = get_string(&front_matter, "area")
        .map(|value| map_area(&value).to_string())
        .unwrap_or_else(|| get_default_area(path, &content).to_string());
    let status = get_string(&front_matter, "status").unwrap_or_else(|| "draft".to_string());

    let mut links = match front_matter.remove(string_value("links")) {
        Some(Value::Mapping(mapping)) => mapping,
        Some(Value::Null) | None => Mapping::new(),
        Some(other) => bail!("links must be a mapping, got {other:?}"),
    };

    for key in ["requirements", "tasks", "adrs"] {
        if let Some(value) = front_matter.remove(string_value(key)) {
            links.insert(string_value(key), value);
        }
    }

    let normalized = NormalizedFrontMatter {
        doc_id: format!("DOC-{}-{}", slugify(&kind), doc_slug(path)?),
        kind,
        area,
        status,
        links: NormalizedLinks {
            requirements: list_from_mapping(&links, "requirements")?,
            tasks: list_from_mapping(&links, "tasks")?,
            adrs: list_from_mapping(&links, "adrs")?,
        },
    };

    let yaml = serde_yaml::to_string(&normalized).context("failed to serialize front matter")?;
    fs::write(path, format!("---\n{yaml}---\n{body}"))
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

fn split_front_matter(content: &str) -> Result<(Mapping, String)> {
    if let Some(stripped) = content.strip_prefix("---\n")
        && let Some(end) = stripped.find("\n---\n")
    {
        let yaml = &stripped[..end];
        let body = stripped[end + "\n---\n".len()..].to_string();
        let front_matter =
            serde_yaml::from_str::<Mapping>(yaml).context("front matter YAML is invalid")?;
        return Ok((front_matter, body));
    }

    Ok((Mapping::new(), content.to_string()))
}

fn get_default_kind(path: &Path) -> &'static str {
    let parts: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
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

fn get_default_area(_path: &Path, content: &str) -> &'static str {
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
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("markdown path has no UTF-8 file stem")?;
    let mut name_slug = slugify(name);

    let docs_dir = Path::new(DOCS_DIR);
    let parent = path.parent().unwrap_or(docs_dir);
    let relative_parent = parent.strip_prefix(docs_dir).unwrap_or(parent);
    let parts: Vec<_> = relative_parent
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();

    if !parts.is_empty() {
        name_slug = format!("{}-{name_slug}", slugify(&parts.join("-")));
    }

    Ok(name_slug)
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for ch in value.chars().flat_map(char::to_uppercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_was_dash = false;
        } else if !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn get_string(mapping: &Mapping, key: &str) -> Option<String> {
    mapping
        .get(string_value(key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn remove_string(mapping: &mut Mapping, key: &str) -> Option<String> {
    mapping
        .remove(string_value(key))
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

fn list_from_mapping(mapping: &Mapping, key: &str) -> Result<Vec<String>> {
    match mapping.get(string_value(key)) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(value)) => Ok(vec![value.clone()]),
        Some(Value::Sequence(values)) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                other => bail!("{key} entries must be strings, got {other:?}"),
            })
            .collect(),
        Some(other) => bail!("{key} must be a string or list, got {other:?}"),
    }
}

fn string_value(value: &str) -> Value {
    Value::String(value.to_string())
}
