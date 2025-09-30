#!/usr/bin/env cargo +nightly -Zscript
//! Breaking change detection script for Flight Hub IPC
//!
//! This script compares the current proto schema with the previous version
//! to detect breaking changes that would require a version bump.

use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        eprintln!("Usage: {} <old-proto-file> <new-proto-file>", args[0]);
        process::exit(1);
    }
    
    let old_proto_path = &args[1];
    let new_proto_path = &args[2];
    
    // Read proto files
    let old_proto = match fs::read_to_string(old_proto_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading old proto file {}: {}", old_proto_path, e);
            process::exit(1);
        }
    };
    
    let new_proto = match fs::read_to_string(new_proto_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading new proto file {}: {}", new_proto_path, e);
            process::exit(1);
        }
    };
    
    // Detect breaking changes
    let breaking_changes = detect_breaking_changes(&old_proto, &new_proto);
    
    if breaking_changes.is_empty() {
        println!("✅ No breaking changes detected");
        process::exit(0);
    } else {
        println!("❌ Breaking changes detected:");
        for change in &breaking_changes {
            println!("  - {}", change);
        }
        
        println!("\n💡 Breaking changes require a major version bump.");
        println!("   Consider adding new fields/RPCs instead of removing existing ones.");
        
        process::exit(1);
    }
}

/// Detect breaking changes between two proto schemas
fn detect_breaking_changes(old_schema: &str, new_schema: &str) -> Vec<String> {
    let mut breaking_changes = Vec::new();
    
    // Extract RPCs from old schema
    let old_rpcs = extract_rpcs(old_schema);
    let new_rpcs = extract_rpcs(new_schema);
    
    // Check for removed RPCs
    for old_rpc in &old_rpcs {
        if !new_rpcs.contains(old_rpc) {
            breaking_changes.push(format!("Removed RPC: {}", old_rpc));
        }
    }
    
    // Extract messages from old schema
    let old_messages = extract_messages(old_schema);
    let new_messages = extract_messages(new_schema);
    
    // Check for removed messages
    for old_message in &old_messages {
        if !new_messages.contains(old_message) {
            breaking_changes.push(format!("Removed message: {}", old_message));
        }
    }
    
    // Extract enums from old schema
    let old_enums = extract_enums(old_schema);
    let new_enums = extract_enums(new_schema);
    
    // Check for removed enums
    for old_enum in &old_enums {
        if !new_enums.contains(old_enum) {
            breaking_changes.push(format!("Removed enum: {}", old_enum));
        }
    }
    
    // Check for removed enum values (simplified)
    for line in old_schema.lines() {
        let trimmed = line.trim();
        if trimmed.contains(" = ") && trimmed.ends_with(';') {
            // This is likely an enum value
            if !new_schema.contains(trimmed) {
                breaking_changes.push(format!("Removed enum value: {}", trimmed));
            }
        }
    }
    
    breaking_changes
}

/// Extract RPC method names from proto schema
fn extract_rpcs(schema: &str) -> Vec<String> {
    let mut rpcs = Vec::new();
    
    for line in schema.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("rpc ") {
            if let Some(rpc_name) = extract_rpc_name(trimmed) {
                rpcs.push(rpc_name);
            }
        }
    }
    
    rpcs
}

/// Extract RPC name from a line like "rpc ListDevices(ListDevicesRequest) returns (ListDevicesResponse);"
fn extract_rpc_name(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "rpc" {
        let name_part = parts[1];
        if let Some(paren_pos) = name_part.find('(') {
            return Some(name_part[..paren_pos].to_string());
        }
    }
    None
}

/// Extract message names from proto schema
fn extract_messages(schema: &str) -> Vec<String> {
    let mut messages = Vec::new();
    
    for line in schema.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("message ") {
            if let Some(message_name) = extract_message_name(trimmed) {
                messages.push(message_name);
            }
        }
    }
    
    messages
}

/// Extract message name from a line like "message Device {"
fn extract_message_name(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "message" {
        let name_part = parts[1];
        if let Some(brace_pos) = name_part.find('{') {
            return Some(name_part[..brace_pos].to_string());
        } else {
            return Some(name_part.to_string());
        }
    }
    None
}

/// Extract enum names from proto schema
fn extract_enums(schema: &str) -> Vec<String> {
    let mut enums = Vec::new();
    
    for line in schema.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("enum ") {
            if let Some(enum_name) = extract_enum_name(trimmed) {
                enums.push(enum_name);
            }
        }
    }
    
    enums
}

/// Extract enum name from a line like "enum DeviceType {"
fn extract_enum_name(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "enum" {
        let name_part = parts[1];
        if let Some(brace_pos) = name_part.find('{') {
            return Some(name_part[..brace_pos].to_string());
        } else {
            return Some(name_part.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_rpcs() {
        let schema = r#"
        service FlightService {
            rpc ListDevices(ListDevicesRequest) returns (ListDevicesResponse);
            rpc HealthSubscribe(HealthSubscribeRequest) returns (stream HealthEvent);
        }
        "#;
        
        let rpcs = extract_rpcs(schema);
        assert_eq!(rpcs, vec!["ListDevices", "HealthSubscribe"]);
    }
    
    #[test]
    fn test_extract_messages() {
        let schema = r#"
        message Device {
            string id = 1;
        }
        
        message ListDevicesRequest {}
        "#;
        
        let messages = extract_messages(schema);
        assert_eq!(messages, vec!["Device", "ListDevicesRequest"]);
    }
    
    #[test]
    fn test_detect_breaking_changes() {
        let old_schema = r#"
        service FlightService {
            rpc ListDevices(ListDevicesRequest) returns (ListDevicesResponse);
            rpc HealthSubscribe(HealthSubscribeRequest) returns (stream HealthEvent);
        }
        "#;
        
        let new_schema = r#"
        service FlightService {
            rpc ListDevices(ListDevicesRequest) returns (ListDevicesResponse);
            // HealthSubscribe removed
        }
        "#;
        
        let changes = detect_breaking_changes(old_schema, new_schema);
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|c| c.contains("HealthSubscribe")));
    }
}