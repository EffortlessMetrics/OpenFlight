# ADR-010: Schema Versioning Strategy

## Status
Accepted

## Context

Flight Hub uses multiple schemas for profiles, IPC, telemetry, and configuration. These schemas evolve over time, requiring careful versioning to maintain compatibility while enabling innovation. Breaking changes must be handled gracefully without disrupting existing users.

## Decision

We implement a comprehensive schema versioning strategy with semantic versioning and migration support:

### 1. Schema Identification

Each schema includes explicit version information:

```json
{
  "schema": "flight.profile/1",
  "version": "1.2.0",
  "data": { ... }
}
```

**Schema URI Format:** `{domain}.{type}/{major_version}`
- `flight.profile/1` - Profile schema major version 1
- `flight.ipc/2` - IPC schema major version 2  
- `flight.telemetry/1` - Telemetry schema major version 1

### 2. Versioning Semantics

**Major Version (Breaking Changes):**
- Field removal or type changes
- Required field additions
- Semantic behavior changes
- Incompatible with previous major version

**Minor Version (Backward Compatible):**
- Optional field additions
- New enum values with defaults
- Extended validation rules
- Compatible with same major version

**Patch Version (Bug Fixes):**
- Documentation clarifications
- Validation bug fixes
- No schema structure changes

### 3. Schema Evolution Matrix

| Change Type | Major | Minor | Patch | Migration Required |
|-------------|-------|-------|-------|-------------------|
| Add optional field | No | Yes | No | No |
| Add required field | Yes | No | No | Yes |
| Remove field | Yes | No | No | Yes |
| Change field type | Yes | No | No | Yes |
| Rename field | Yes | No | No | Yes |
| Add enum value | No | Yes | No | No |
| Remove enum value | Yes | No | No | Yes |
| Tighten validation | No | Yes | No | No |
| Loosen validation | No | Yes | No | No |

### 4. Migration Framework

```rust
pub trait SchemaMigrator {
    fn can_migrate(&self, from: &Version, to: &Version) -> bool;
    fn migrate(&self, data: &Value, from: &Version, to: &Version) -> Result<Value>;
    fn migration_path(&self, from: &Version, to: &Version) -> Vec<Version>;
}

pub struct ProfileMigrator {
    migrations: BTreeMap<(Version, Version), Box<dyn Migration>>,
}

impl ProfileMigrator {
    pub fn migrate_profile(&self, profile: &str) -> Result<String> {
        let parsed: ProfileDocument = serde_json::from_str(profile)?;
        let current_version = Version::parse(&parsed.version)?;
        let target_version = CURRENT_PROFILE_VERSION;
        
        if current_version == target_version {
            return Ok(profile.to_string());
        }
        
        let path = self.migration_path(&current_version, &target_version)?;
        let mut data = parsed.data;
        
        for step in path.windows(2) {
            let migration = self.migrations.get(&(step[0], step[1]))
                .ok_or_else(|| Error::NoMigrationPath)?;
            data = migration.apply(data)?;
        }
        
        Ok(serde_json::to_string_pretty(&ProfileDocument {
            schema: format!("flight.profile/{}", target_version.major),
            version: target_version.to_string(),
            data,
        })?)
    }
}
```

### 5. Compatibility Matrix

```rust
pub struct CompatibilityMatrix {
    // Supported version ranges for each schema type
    profile_versions: VersionRange,
    ipc_versions: VersionRange,
    telemetry_versions: VersionRange,
}

impl CompatibilityMatrix {
    pub fn is_compatible(&self, schema_type: SchemaType, version: &Version) -> bool {
        match schema_type {
            SchemaType::Profile => self.profile_versions.contains(version),
            SchemaType::Ipc => self.ipc_versions.contains(version),
            SchemaType::Telemetry => self.telemetry_versions.contains(version),
        }
    }
    
    pub fn migration_required(&self, schema_type: SchemaType, version: &Version) -> bool {
        !self.is_compatible(schema_type, version) && 
        self.can_migrate(schema_type, version)
    }
}
```

## Consequences

### Positive
- Clear compatibility guarantees for users
- Automated migration reduces upgrade friction
- Explicit versioning prevents silent breakage
- CI validation catches breaking changes

### Negative
- Increased complexity in schema management
- Migration code maintenance burden
- Potential performance overhead during migration
- Storage overhead for version metadata

## Alternatives Considered

1. **No Versioning**: Rejected due to inevitable breaking changes
2. **Date-Based Versioning**: Rejected due to unclear compatibility semantics
3. **Git Hash Versioning**: Rejected due to poor user experience
4. **Single Global Version**: Rejected due to coupling between unrelated schemas

## Implementation Details

### Schema Validation

```rust
pub struct SchemaValidator {
    schemas: HashMap<String, JsonSchema>,
}

impl SchemaValidator {
    pub fn validate(&self, document: &Value) -> Result<()> {
        let schema_uri = document["schema"].as_str()
            .ok_or(Error::MissingSchemaField)?;
        
        let schema = self.schemas.get(schema_uri)
            .ok_or(Error::UnknownSchema(schema_uri.to_string()))?;
        
        let validation_result = schema.validate(document);
        if let Err(errors) = validation_result {
            return Err(Error::ValidationFailed(errors));
        }
        
        Ok(())
    }
}
```

### Migration Examples

**Profile Schema 1.0 → 1.1 (Add optional field):**
```rust
pub struct AddOptionalFieldMigration;

impl Migration for AddOptionalFieldMigration {
    fn apply(&self, mut data: Value) -> Result<Value> {
        // Add default value for new optional field
        if !data["axes"].as_object().unwrap().contains_key("mixers") {
            data["axes"]["mixers"] = json!([]);
        }
        Ok(data)
    }
}
```

**Profile Schema 1.1 → 2.0 (Breaking change):**
```rust
pub struct CurveFormatMigration;

impl Migration for CurveFormatMigration {
    fn apply(&self, mut data: Value) -> Result<Value> {
        // Convert old curve format to new format
        if let Some(axes) = data["axes"].as_object_mut() {
            for axis in axes.values_mut() {
                if let Some(curve) = axis.get_mut("curve") {
                    // Old format: {"type": "expo", "value": 0.5}
                    // New format: {"expo": 0.5}
                    if let Some(curve_type) = curve["type"].as_str() {
                        match curve_type {
                            "expo" => {
                                let value = curve["value"].clone();
                                *curve = json!({"expo": value});
                            },
                            "linear" => {
                                *curve = json!({"linear": {}});
                            },
                            _ => return Err(Error::UnknownCurveType),
                        }
                    }
                }
            }
        }
        Ok(data)
    }
}
```

### CI Integration

```yaml
# Schema validation in CI
- name: Validate Schema Changes
  run: |
    # Check for breaking changes
    cargo run --bin schema-diff -- \
      --old schemas/v1.0.0/ \
      --new schemas/v1.1.0/ \
      --fail-on-breaking
    
    # Validate migration paths
    cargo test schema_migrations
    
    # Test backward compatibility
    cargo run --bin compatibility-test
```

## Testing Strategy

### Unit Tests
- Migration correctness for each version pair
- Schema validation with valid/invalid documents
- Compatibility matrix logic
- Version parsing and comparison

### Integration Tests
- End-to-end migration scenarios
- Cross-version compatibility testing
- Performance impact of migrations
- Error handling for unsupported versions

### Property Tests
- Migration idempotency (migrate twice = migrate once)
- Round-trip compatibility where possible
- Version ordering consistency

## Error Handling

### Migration Failures
- Clear error messages with context
- Rollback to original document on failure
- Detailed logging for debugging
- User-friendly error reporting

### Unsupported Versions
- Clear indication of version support status
- Guidance on upgrade path
- Fallback to safe defaults where possible

## Performance Considerations

- Lazy migration (only when needed)
- Caching of migrated documents
- Streaming migration for large documents
- Parallel migration for batch operations

## Documentation Requirements

- Migration guide for each major version
- Compatibility matrix published with releases
- Schema change log with rationale
- Examples for common migration scenarios

## References

- Flight Hub Requirements: IFC-01, PRF-01
- [Semantic Versioning](https://semver.org/)
- [JSON Schema Specification](https://json-schema.org/)
- [Database Migration Patterns](https://example.com)