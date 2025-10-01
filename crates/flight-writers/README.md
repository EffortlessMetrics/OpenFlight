# Flight Writers

A table-driven configuration management system for flight simulators with golden file testing, verify/repair functionality, and one-click rollback capabilities.

## Overview

Flight Writers provides a robust system for managing simulator configurations through:

- **Versioned JSON Diffs**: Table-driven configuration changes per simulator version
- **Golden File Testing**: Automated testing with expected output validation
- **Verify/Repair**: Scripted verification and minimal diff repair system
- **One-Click Rollback**: Complete backup and restoration functionality
- **Coverage Matrix**: Comprehensive testing coverage reporting

## Features

### Configuration Management
- **Multiple Operation Types**: INI sections, JSON patches, line replacements, full file replacements
- **Atomic Operations**: All changes are applied atomically with automatic backup
- **Version-Specific**: Configurations are tied to specific simulator versions
- **Cross-Platform**: Works on Windows and Linux

### Testing & Validation
- **Golden File Tests**: Compare actual output against expected golden files
- **Verification Scripts**: Run scripted tests to validate configuration effectiveness
- **Coverage Reporting**: Track which simulator versions and areas are tested
- **CI Integration**: Fail builds on golden file mismatches

### Safety & Recovery
- **Automatic Backups**: Every change creates a timestamped backup
- **Integrity Verification**: SHA-256 checksums ensure backup integrity
- **One-Click Rollback**: Restore previous configurations instantly
- **Minimal Repairs**: Apply only the changes needed to fix issues

## Usage

### Basic API Usage

```rust
use flight_writers::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Writers instance
    let writers = Writers::new("./config", "./golden", "./backups")?;
    
    // Load and apply a configuration
    let config: WriterConfig = serde_json::from_str(&config_json)?;
    let result = writers.apply_writer(&config).await?;
    
    if result.success {
        println!("Applied {} changes, backup: {}", 
                 result.modified_files.len(), result.backup_id);
    }
    
    // Verify configuration
    let verify_result = writers.verify(SimulatorType::MSFS, "1.36.0").await?;
    
    // Repair if needed
    if !verify_result.success {
        let repair_result = writers.repair(&verify_result).await?;
        println!("Repaired {} files", repair_result.repaired_files.len());
    }
    
    // Run golden file tests
    let test_result = writers.test_golden_files(SimulatorType::MSFS).await?;
    println!("Tests: {}/{} passed, Coverage: {:.1}%", 
             test_result.test_cases.iter().filter(|t| t.success).count(),
             test_result.test_cases.len(),
             test_result.coverage.coverage_percent);
    
    Ok(())
}
```

### Configuration Format

```json
{
  "schema": "flight.writer/1",
  "sim": "msfs",
  "version": "1.36.0",
  "description": "Enhanced autopilot configuration",
  "diffs": [
    {
      "file": "SimObjects/Airplanes/C172/panel.cfg",
      "type": "ini_section",
      "section": "AUTOPILOT",
      "changes": {
        "autopilot_available": "1",
        "flight_director_available": "1",
        "default_vertical_speed": "700"
      },
      "backup": true
    },
    {
      "file": "config.json",
      "type": "json_patch",
      "patches": [
        {
          "op": "add",
          "path": "/features/enhanced_ap",
          "value": true
        }
      ],
      "backup": true
    }
  ],
  "verify_scripts": [
    {
      "name": "autopilot_test",
      "description": "Test autopilot engagement",
      "actions": [
        {
          "type": "sim_event",
          "event": "AP_MASTER",
          "value": 1
        },
        {
          "type": "wait",
          "duration_ms": 500
        }
      ],
      "expected": [
        {
          "variable": "AUTOPILOT_MASTER",
          "value": 1.0,
          "tolerance": 0.1
        }
      ]
    }
  ]
}
```

### Golden File Testing

Golden file tests validate that configurations produce expected outputs:

```
golden/
├── msfs/
│   ├── test_v1.36.0_autopilot/
│   │   ├── input.json          # Writer configuration
│   │   ├── expected/           # Expected output files
│   │   │   └── panel.cfg
│   │   └── actual/             # Generated during test
│   └── test_v1.36.0_electrical/
│       ├── input.json
│       └── expected/
│           └── electrical.cfg
└── xplane/
    └── test_v12.1.0_lighting/
        ├── input.json
        └── expected/
            └── lighting.cfg
```

### CLI Usage

```bash
# Apply a configuration
writers-cli apply config.json

# Verify current state
writers-cli verify msfs 1.36.0

# Repair configuration
writers-cli repair msfs 1.36.0

# Run golden file tests
writers-cli test msfs

# Generate coverage report
writers-cli coverage msfs

# List available backups
writers-cli list-backups

# Rollback to previous state
writers-cli rollback backup_1234567890
```

## Operation Types

### INI Section Modification
Modify specific sections in INI-style configuration files:

```json
{
  "type": "ini_section",
  "section": "AUTOPILOT",
  "changes": {
    "autopilot_available": "1",
    "flight_director_available": "1"
  }
}
```

### JSON Patch
Apply RFC 6902 JSON Patch operations:

```json
{
  "type": "json_patch",
  "patches": [
    {
      "op": "add",
      "path": "/new_feature",
      "value": {"enabled": true}
    },
    {
      "op": "replace",
      "path": "/existing_value",
      "value": 42
    }
  ]
}
```

### Line Replacement
Replace lines using string or regex matching:

```json
{
  "type": "line_replace",
  "pattern": "old_setting=.*",
  "replacement": "old_setting=new_value",
  "regex": true
}
```

### File Replacement
Replace entire file contents:

```json
{
  "type": "replace",
  "content": "# New file content\nkey=value\n"
}
```

## Verification Scripts

Verification scripts test that configurations work correctly:

### Actions
- **sim_event**: Send simulator events
- **wait**: Wait for specified duration
- **check_var**: Verify simulator variable values

### Example
```json
{
  "name": "gear_test",
  "description": "Test landing gear operation",
  "actions": [
    {"type": "sim_event", "event": "GEAR_TOGGLE"},
    {"type": "wait", "duration_ms": 1000},
    {"type": "check_var", "variable": "GEAR_POSITION", "expected": 1.0}
  ],
  "expected": [
    {
      "variable": "GEAR_POSITION",
      "value": 1.0,
      "tolerance": 0.1
    }
  ]
}
```

## Coverage Matrix

The coverage matrix tracks testing completeness:

- **Versions**: Which simulator versions are covered
- **Areas**: Which configuration areas are tested (autopilot, electrical, etc.)
- **Percentage**: Overall coverage percentage
- **Missing**: Areas that need additional coverage

## CI Integration

Golden file tests can be integrated into CI pipelines:

```yaml
- name: Run Golden File Tests
  run: |
    cargo run --example writers_cli -- test msfs
    cargo run --example writers_cli -- test xplane
    cargo run --example writers_cli -- test dcs
```

Tests fail if:
- Golden file outputs don't match expected results
- Coverage drops below threshold
- Verification scripts fail

## Safety Features

### Automatic Backups
Every configuration change creates a backup with:
- Unique timestamp-based ID
- SHA-256 checksums for integrity
- Complete file restoration capability
- Metadata tracking (sim, version, description)

### Atomic Operations
All changes are applied atomically:
- Either all files are modified successfully, or none are
- Partial failures are rolled back automatically
- Backup creation happens before any modifications

### Integrity Verification
Backups are verified using:
- SHA-256 checksums of original files
- File existence validation
- Restoration testing

## Error Handling

The system provides comprehensive error handling:

- **Stable Error Codes**: Consistent error identification
- **Detailed Context**: Line/column information for JSON/INI errors
- **Graceful Degradation**: Continue processing other files on individual failures
- **Recovery Suggestions**: Automatic repair recommendations

## Performance

- **Minimal I/O**: Only modified files are written
- **Efficient Diffs**: Calculate minimal changes needed
- **Parallel Processing**: Independent operations run concurrently
- **Memory Efficient**: Stream large files instead of loading entirely

## Requirements Satisfied

This implementation satisfies the following requirements from the Flight Hub specification:

- **GI-01**: Multi-simulator support with versioned configurations
- **GI-02**: Verify/Repair matrix with scripted validation
- **GI-05**: One-click rollback with comprehensive backup system

The system provides table-driven configuration management with golden file testing, comprehensive verification, and robust rollback capabilities as specified in task 17 of the implementation plan.