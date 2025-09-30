#!/usr/bin/env cargo +stable -Zscript

//! Validates that all ADR links in README files are valid and point to existing files.
//! 
//! Usage: cargo +stable -Zscript scripts/validate_adr_links.rs

use std::fs;
use std::path::Path;
use regex::Regex;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Validating ADR links in README files...");
    
    let adr_link_regex = Regex::new(r"\[ADR-\d+[^\]]*\]\(([^)]+)\)")?;
    let mut errors = Vec::new();
    let mut total_links = 0;
    
    // Find all README files
    let readme_files = find_readme_files(".")?;
    
    for readme_path in readme_files {
        println!("📄 Checking {}", readme_path);
        
        let content = fs::read_to_string(&readme_path)?;
        
        for captures in adr_link_regex.captures_iter(&content) {
            total_links += 1;
            let link_path = &captures[1];
            
            // Resolve relative path from README location
            let readme_dir = Path::new(&readme_path).parent().unwrap();
            let full_path = readme_dir.join(link_path);
            
            if !full_path.exists() {
                errors.push(format!("❌ Broken ADR link in {}: {} -> {}", 
                    readme_path, link_path, full_path.display()));
            } else {
                println!("  ✅ {}", link_path);
            }
        }
    }
    
    println!("\n📊 Summary:");
    println!("  Total ADR links found: {}", total_links);
    println!("  Broken links: {}", errors.len());
    
    if !errors.is_empty() {
        println!("\n💥 Errors found:");
        for error in errors {
            println!("  {}", error);
        }
        std::process::exit(1);
    }
    
    println!("✅ All ADR links are valid!");
    Ok(())
}

fn find_readme_files(dir: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut readme_files = Vec::new();
    
    fn visit_dir(dir: &Path, readme_files: &mut Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    // Skip target and .git directories
                    if let Some(name) = path.file_name() {
                        if name == "target" || name == ".git" {
                            continue;
                        }
                    }
                    visit_dir(&path, readme_files)?;
                } else if let Some(name) = path.file_name() {
                    if name == "README.md" {
                        readme_files.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
        Ok(())
    }
    
    visit_dir(Path::new(dir), &mut readme_files)?;
    Ok(readme_files)
}

// Cargo.toml for this script
/*
[dependencies]
regex = "1.10"
*/