#!/usr/bin/env cargo +nightly -Zscript
//! Security Verification Script for CI/Manual Checks
//!
//! This script performs comprehensive security verification according to SEC-01 requirements.
//! It can be run in CI or manually to validate the security posture of Flight Hub.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, exit};

fn main() {
    println!("🔒 Flight Hub Security Verification");
    println!("==================================");
    
    let mut all_passed = true;
    let mut results = HashMap::new();
    
    // Check 1: IPC Local-Only Verification
    println!("\n📡 Checking IPC Configuration...");
    let ipc_result = verify_ipc_local_only();
    results.insert("ipc_local_only", ipc_result);
    if !ipc_result {
        all_passed = false;
        eprintln!("❌ IPC local-only check failed");
    } else {
        println!("✅ IPC is properly configured for local-only access");
    }
    
    // Check 2: Plugin Signing Surface
    println!("\n🔐 Checking Plugin Signing Configuration...");
    let signing_result = verify_plugin_signing_surface();
    results.insert("plugin_signing", signing_result);
    if !signing_result {
        all_passed = false;
        eprintln!("❌ Plugin signing surface check failed");
    } else {
        println!("✅ Plugin signing surface is properly implemented");
    }
    
    // Check 3: Telemetry Privacy
    println!("\n🕵️ Checking Telemetry Privacy Configuration...");
    let telemetry_result = verify_telemetry_privacy();
    results.insert("telemetry_privacy", telemetry_result);
    if !telemetry_result {
        all_passed = false;
        eprintln!("❌ Telemetry privacy check failed");
    } else {
        println!("✅ Telemetry privacy is properly configured");
    }
    
    // Check 4: No Network Listeners by Default
    println!("\n🌐 Checking Network Listener Configuration...");
    let network_result = verify_no_network_listeners();
    results.insert("no_network_listeners", network_result);
    if !network_result {
        all_passed = false;
        eprintln!("❌ Network listener check failed");
    } else {
        println!("✅ No unauthorized network listeners detected");
    }
    
    // Check 5: Code Injection Prevention
    println!("\n💉 Checking Code Injection Prevention...");
    let injection_result = verify_no_code_injection();
    results.insert("no_code_injection", injection_result);
    if !injection_result {
        all_passed = false;
        eprintln!("❌ Code injection prevention check failed");
    } else {
        println!("✅ Code injection prevention is properly implemented");
    }
    
    // Check 6: Capability Validation
    println!("\n🛡️ Checking Plugin Capability Validation...");
    let capability_result = verify_capability_validation();
    results.insert("capability_validation", capability_result);
    if !capability_result {
        all_passed = false;
        eprintln!("❌ Plugin capability validation check failed");
    } else {
        println!("✅ Plugin capability validation is properly implemented");
    }
    
    // Check 7: Secure Defaults
    println!("\n⚙️ Checking Secure Configuration Defaults...");
    let defaults_result = verify_secure_defaults();
    results.insert("secure_defaults", defaults_result);
    if !defaults_result {
        all_passed = false;
        eprintln!("❌ Secure defaults check failed");
    } else {
        println!("✅ Secure configuration defaults are properly set");
    }
    
    // Check 8: Audit Logging
    println!("\n📋 Checking Audit Logging Configuration...");
    let audit_result = verify_audit_logging();
    results.insert("audit_logging", audit_result);
    if !audit_result {
        all_passed = false;
        eprintln!("❌ Audit logging check failed");
    } else {
        println!("✅ Audit logging is properly configured");
    }
    
    // Summary
    println!("\n📊 Security Verification Summary");
    println!("===============================");
    
    let passed_count = results.values().filter(|&&v| v).count();
    let total_count = results.len();
    
    println!("Checks passed: {}/{}", passed_count, total_count);
    
    for (check_name, passed) in &results {
        let status = if *passed { "✅ PASS" } else { "❌ FAIL" };
        println!("  {} {}", status, check_name);
    }
    
    if all_passed {
        println!("\n🎉 All security checks passed!");
        exit(0);
    } else {
        println!("\n💥 Some security checks failed!");
        
        // Print remediation guidance
        println!("\n🔧 Remediation Guidance:");
        print_remediation_guidance(&results);
        
        exit(1);
    }
}

fn verify_ipc_local_only() -> bool {
    // Check that IPC configuration only uses local transports
    
    // 1. Check flight-ipc source code for network bindings
    if let Ok(transport_code) = fs::read_to_string("crates/flight-ipc/src/transport.rs") {
        // Should not contain TCP/network binding code
        if transport_code.contains("TcpListener") || transport_code.contains("bind(\"0.0.0.0") {
            eprintln!("  ❌ Found potential network binding in transport code");
            return false;
        }
    }
    
    // 2. Check server configuration
    if let Ok(server_code) = fs::read_to_string("crates/flight-ipc/src/server.rs") {
        // Should only bind to localhost or use named pipes/UDS
        if server_code.contains("0.0.0.0") && !server_code.contains("// For development") {
            eprintln!("  ❌ Found non-localhost binding in server code");
            return false;
        }
    }
    
    // 3. Check default configuration
    if let Ok(lib_code) = fs::read_to_string("crates/flight-ipc/src/lib.rs") {
        // Default addresses should be local-only
        if lib_code.contains("default_bind_address") {
            if !lib_code.contains("pipe\\flight-hub") && !lib_code.contains("/tmp/flight-hub.sock") {
                eprintln!("  ❌ Default bind address is not local-only");
                return false;
            }
        }
    }
    
    true
}

fn verify_plugin_signing_surface() -> bool {
    // Check that plugin signing infrastructure is implemented
    
    // 1. Check security module exists
    if !Path::new("crates/flight-core/src/security.rs").exists() {
        eprintln!("  ❌ Security module not found");
        return false;
    }
    
    // 2. Check for signature verification code
    if let Ok(security_code) = fs::read_to_string("crates/flight-core/src/security.rs") {
        if !security_code.contains("SignatureStatus") || !security_code.contains("verify_plugin_signature") {
            eprintln!("  ❌ Plugin signature verification not implemented");
            return false;
        }
    }
    
    // 3. Check for capability manifest validation
    if let Ok(security_code) = fs::read_to_string("crates/flight-core/src/security.rs") {
        if !security_code.contains("PluginCapabilityManifest") || !security_code.contains("validate_capabilities") {
            eprintln!("  ❌ Plugin capability validation not implemented");
            return false;
        }
    }
    
    true
}

fn verify_telemetry_privacy() -> bool {
    // Check that telemetry is opt-in and privacy-preserving
    
    // 1. Check default telemetry configuration
    if let Ok(security_code) = fs::read_to_string("crates/flight-core/src/security.rs") {
        if !security_code.contains("enabled: false") {
            eprintln!("  ❌ Telemetry is not disabled by default");
            return false;
        }
    }
    
    // 2. Check for data redaction in support bundles
    if let Ok(security_code) = fs::read_to_string("crates/flight-core/src/security.rs") {
        if !security_code.contains("get_redacted_support_data") || !security_code.contains("[REDACTED]") {
            eprintln!("  ❌ Support data redaction not implemented");
            return false;
        }
    }
    
    true
}

fn verify_no_network_listeners() -> bool {
    // Check that no network listeners are started by default
    
    // 1. Check server implementation
    if let Ok(server_code) = fs::read_to_string("crates/flight-ipc/src/server.rs") {
        // Should not bind to network interfaces by default
        if server_code.contains("0.0.0.0:") && !server_code.contains("For development") {
            eprintln!("  ❌ Found network listener binding");
            return false;
        }
    }
    
    // 2. Check for explicit network binding prevention
    if let Ok(transport_code) = fs::read_to_string("crates/flight-ipc/src/transport.rs") {
        // Should have local-only transport types
        if !transport_code.contains("NamedPipes") || !transport_code.contains("UnixSockets") {
            eprintln!("  ❌ Local-only transport types not found");
            return false;
        }
    }
    
    true
}

fn verify_no_code_injection() -> bool {
    // Check that no code injection into sim processes occurs
    
    // 1. Check sim adapter implementations
    let sim_crates = ["flight-simconnect", "flight-xplane", "flight-dcs-export"];
    
    for crate_name in &sim_crates {
        let crate_path = format!("crates/{}/src/lib.rs", crate_name);
        if Path::new(&crate_path).exists() {
            if let Ok(code) = fs::read_to_string(&crate_path) {
                // Should not contain DLL injection or process manipulation
                if code.contains("CreateRemoteThread") || code.contains("WriteProcessMemory") || code.contains("SetWindowsHookEx") {
                    eprintln!("  ❌ Found potential code injection in {}", crate_name);
                    return false;
                }
            }
        }
    }
    
    true
}

fn verify_capability_validation() -> bool {
    // Check that plugin capability validation is implemented
    
    if let Ok(security_code) = fs::read_to_string("crates/flight-core/src/security.rs") {
        // Should have capability checking
        if !security_code.contains("check_capability") || !security_code.contains("PluginCapability") {
            eprintln!("  ❌ Plugin capability checking not implemented");
            return false;
        }
        
        // Should validate WASM vs Native capabilities
        if !security_code.contains("PluginType::Wasm") || !security_code.contains("FileSystem") {
            eprintln!("  ❌ WASM/Native capability differentiation not implemented");
            return false;
        }
    }
    
    true
}

fn verify_secure_defaults() -> bool {
    // Check that security-sensitive settings have secure defaults
    
    if let Ok(security_code) = fs::read_to_string("crates/flight-core/src/security.rs") {
        // Security config should have secure defaults
        if !security_code.contains("enforce_signatures: true") || !security_code.contains("allow_unsigned: false") {
            eprintln!("  ❌ Security configuration does not have secure defaults");
            return false;
        }
        
        // ACL config should default to current user only
        if !security_code.contains("current_user_only: true") {
            eprintln!("  ❌ ACL configuration does not default to current user only");
            return false;
        }
    }
    
    true
}

fn verify_audit_logging() -> bool {
    // Check that audit logging is implemented
    
    // 1. Check for audit event types
    if let Ok(verification_code) = fs::read_to_string("crates/flight-core/src/security/verification.rs") {
        if !verification_code.contains("AuditEvent") || !verification_code.contains("audit_event") {
            eprintln!("  ❌ Audit logging not implemented");
            return false;
        }
    }
    
    // 2. Check for security event logging in IPC
    if let Ok(server_code) = fs::read_to_string("crates/flight-ipc/src/server.rs") {
        if !server_code.contains("security_manager") {
            eprintln!("  ❌ Security manager not integrated into IPC server");
            return false;
        }
    }
    
    true
}

fn print_remediation_guidance(results: &HashMap<&str, bool>) {
    for (check_name, passed) in results {
        if !*passed {
            match *check_name {
                "ipc_local_only" => {
                    println!("  🔧 IPC Local-Only:");
                    println!("     - Remove any TCP/network binding code from transport layer");
                    println!("     - Ensure default bind addresses use named pipes (Windows) or Unix sockets (Linux)");
                    println!("     - Verify no 0.0.0.0 bindings outside of development code");
                }
                "plugin_signing" => {
                    println!("  🔧 Plugin Signing:");
                    println!("     - Implement SignatureStatus enum and verification logic");
                    println!("     - Add plugin capability manifest validation");
                    println!("     - Create certificate authority trust chain validation");
                }
                "telemetry_privacy" => {
                    println!("  🔧 Telemetry Privacy:");
                    println!("     - Ensure telemetry is disabled by default (enabled: false)");
                    println!("     - Implement data redaction for support bundles");
                    println!("     - Add explicit user consent mechanisms");
                }
                "no_network_listeners" => {
                    println!("  🔧 Network Listeners:");
                    println!("     - Remove any network listener bindings from server code");
                    println!("     - Ensure only local transports (pipes/UDS) are used");
                    println!("     - Add explicit checks to prevent network binding");
                }
                "no_code_injection" => {
                    println!("  🔧 Code Injection Prevention:");
                    println!("     - Remove any DLL injection or process manipulation code");
                    println!("     - Use only approved integration methods (SimConnect, DataRefs, Export.lua)");
                    println!("     - Avoid Windows API calls like CreateRemoteThread, WriteProcessMemory");
                }
                "capability_validation" => {
                    println!("  🔧 Capability Validation:");
                    println!("     - Implement plugin capability checking logic");
                    println!("     - Add WASM vs Native plugin capability differentiation");
                    println!("     - Create capability manifest enforcement");
                }
                "secure_defaults" => {
                    println!("  🔧 Secure Defaults:");
                    println!("     - Set enforce_signatures: true by default");
                    println!("     - Set allow_unsigned: false by default");
                    println!("     - Set current_user_only: true for ACL config");
                }
                "audit_logging" => {
                    println!("  🔧 Audit Logging:");
                    println!("     - Implement AuditEvent structure and logging");
                    println!("     - Integrate security manager into IPC server");
                    println!("     - Add audit event generation for security-sensitive operations");
                }
                _ => {}
            }
        }
    }
    
    println!("\n📚 For more information, see the SEC-01 requirements in .kiro/specs/flight-hub/requirements.md");
}