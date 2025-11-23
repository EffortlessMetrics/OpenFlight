# Test Fixtures

This directory contains test fixtures for validating the xtask validation framework.

## Directory Structure

### `minimal/`
Contains a valid minimal setup that should pass all schema validations:
- `specs/spec_ledger.yaml` - Minimal valid spec ledger with one requirement
- `docs/concepts/flight-core.md` - Minimal valid documentation with front matter
- `infra/local/invariants.yaml` - Minimal valid infrastructure invariants

### `invalid/schema_errors/`
Contains various schema violations for testing error detection:

#### Spec Ledger Errors
- `spec_ledger_missing_required.yaml` - Missing required 'status' field
- `spec_ledger_invalid_id_pattern.yaml` - Invalid requirement ID pattern
- `spec_ledger_additional_properties.yaml` - Additional properties not allowed
- `spec_ledger_empty_test_object.yaml` - Test object with no properties (violates minProperties: 1)

#### Front Matter Errors
- `front_matter_missing_links.yaml` - Missing required 'links' field
- `front_matter_invalid_kind.yaml` - Invalid kind value (not in enum)
- `front_matter_additional_properties.yaml` - Additional properties not allowed

#### Invariants Errors
- `invariants_missing_rust_version.yaml` - Missing required 'rust_version' field
- `invariants_invalid_port_name.yaml` - Port names don't match pattern
- `invariants_invalid_env_var_name.yaml` - Environment variable names don't match pattern

## Usage

These fixtures are used by the xtask test suite to verify:
1. Valid configurations pass schema validation
2. Invalid configurations are correctly rejected
3. Error messages include helpful information (file path, line number, expected vs found)
