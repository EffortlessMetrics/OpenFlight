#!/usr/bin/env cargo +nightly -Zscript
//! Integration documentation validation script
//! 
//! This script validates that all integration documentation links are valid
//! and that the documentation covers all required aspects.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ValidationError {
    file: PathBuf,
    line: usize,
    message: String,
}

#[derive(Debug)]
struct DocValidation {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationError>,
}

impl DocValidation {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn add_error(&mut self, file: PathBuf, line: usize, message: String) {
        self.errors.push(ValidationError { file, line, message });
    }

    fn add_warning(&mut self, file: PathBuf, line: usize, message: String) {
        self.warnings.push(ValidationError { file, line, message });
    }

    fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut validation = DocValidation::new();
    
    // Validate integration documentation
    validate_integration_docs(&mut validation)?;
    
    // Validate links in documentation
    validate_doc_links(&mut validation)?;
    
    // Validate required sections
    validate_required_sections(&mut validation)?;
    
    // Print results
    print_validation_results(&validation);
    
    if !validation.is_valid() {
        std::process::exit(1);
    }
    
    Ok(())
}

fn validate_integration_docs(validation: &mut DocValidation) -> Result<(), Box<dyn std::error::Error>> {
    let docs_dir = Path::new("docs/integration");
    
    if !docs_dir.exists() {
        validation.add_error(
            docs_dir.to_path_buf(),
            0,
            "Integration documentation directory does not exist".to_string(),
        );
        return Ok(());
    }
    
    // Required documentation files
    let required_files = vec![
        "README.md",
        "msfs.md", 
        "xplane.md",
        "dcs.md",
    ];
    
    for file in required_files {
        let file_path = docs_dir.join(file);
        if !file_path.exists() {
            validation.add_error(
                file_path,
                0,
                format!("Required integration documentation file '{}' is missing", file),
            );
        }
    }
    
    Ok(())
}

fn validate_doc_links(validation: &mut DocValidation) -> Result<(), Box<dyn std::error::Error>> {
    let docs_dir = Path::new("docs/integration");
    
    if !docs_dir.exists() {
        return Ok(());
    }
    
    for entry in fs::read_dir(docs_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            validate_markdown_links(&path, validation)?;
        }
    }
    
    Ok(())
}

fn validate_markdown_links(file_path: &Path, validation: &mut DocValidation) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    
    for (line_num, line) in content.lines().enumerate() {
        // Check for markdown links [text](url)
        let mut chars = line.chars().peekable();
        let mut pos = 0;
        
        while let Some(ch) = chars.next() {
            pos += 1;
            if ch == '[' {
                // Found potential link start
                if let Some(link_end) = find_link_end(&line[pos-1..]) {
                    let link_text = &line[pos-1..pos-1+link_end];
                    if let Some((text, url)) = parse_markdown_link(link_text) {
                        validate_link_target(file_path, line_num + 1, &url, validation);
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn find_link_end(text: &str) -> Option<usize> {
    let mut bracket_count = 0;
    let mut in_url = false;
    
    for (i, ch) in text.chars().enumerate() {
        match ch {
            '[' => bracket_count += 1,
            ']' => {
                bracket_count -= 1;
                if bracket_count == 0 && text.chars().nth(i + 1) == Some('(') {
                    in_url = true;
                }
            }
            ')' if in_url => return Some(i + 1),
            _ => {}
        }
    }
    
    None
}

fn parse_markdown_link(text: &str) -> Option<(String, String)> {
    if let Some(bracket_end) = text.find("](") {
        if let Some(paren_end) = text.rfind(')') {
            let link_text = text[1..bracket_end].to_string();
            let url = text[bracket_end + 2..paren_end].to_string();
            return Some((link_text, url));
        }
    }
    None
}

fn validate_link_target(file_path: &Path, line: usize, url: &str, validation: &mut DocValidation) {
    // Skip external URLs
    if url.starts_with("http://") || url.starts_with("https://") {
        return;
    }
    
    // Skip anchors for now (would need more complex validation)
    if url.starts_with('#') {
        return;
    }
    
    // Resolve relative path
    let base_dir = file_path.parent().unwrap_or(Path::new("."));
    let target_path = base_dir.join(url);
    
    if !target_path.exists() {
        validation.add_error(
            file_path.to_path_buf(),
            line,
            format!("Link target '{}' does not exist", url),
        );
    }
}

fn validate_required_sections(validation: &mut DocValidation) -> Result<(), Box<dyn std::error::Error>> {
    let docs_dir = Path::new("docs/integration");
    
    // Required sections for each simulator doc
    let required_sections = vec![
        ("msfs.md", vec![
            "## Overview",
            "## Files Modified", 
            "## Network Connections",
            "## Variables Accessed",
            "## Revert Steps",
            "## What Flight Hub Does NOT Touch",
        ]),
        ("xplane.md", vec![
            "## Overview",
            "## Files Modified",
            "## Network Connections", 
            "## DataRefs Accessed",
            "## Revert Steps",
            "## What Flight Hub Does NOT Touch",
        ]),
        ("dcs.md", vec![
            "## Overview",
            "## Files Modified",
            "## Network Connections",
            "## Data Accessed",
            "## Multiplayer Integrity",
            "## Revert Steps",
            "## What Flight Hub Does NOT Touch",
        ]),
    ];
    
    for (file_name, sections) in required_sections {
        let file_path = docs_dir.join(file_name);
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            
            for section in sections {
                if !content.contains(section) {
                    validation.add_error(
                        file_path.clone(),
                        0,
                        format!("Required section '{}' is missing", section),
                    );
                }
            }
        }
    }
    
    Ok(())
}

fn print_validation_results(validation: &DocValidation) {
    if validation.errors.is_empty() && validation.warnings.is_empty() {
        println!("✅ All integration documentation validation checks passed!");
        return;
    }
    
    if !validation.errors.is_empty() {
        println!("❌ Documentation validation errors:");
        for error in &validation.errors {
            println!("  {}:{} - {}", 
                error.file.display(), 
                error.line, 
                error.message
            );
        }
        println!();
    }
    
    if !validation.warnings.is_empty() {
        println!("⚠️  Documentation validation warnings:");
        for warning in &validation.warnings {
            println!("  {}:{} - {}", 
                warning.file.display(), 
                warning.line, 
                warning.message
            );
        }
        println!();
    }
    
    let total_issues = validation.errors.len() + validation.warnings.len();
    println!("Total issues found: {} ({} errors, {} warnings)", 
        total_issues, 
        validation.errors.len(), 
        validation.warnings.len()
    );
}