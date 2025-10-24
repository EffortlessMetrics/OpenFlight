# Regression Prevention Quick Reference

Quick reference card for OpenFlight regression prevention measures.

## 🚀 Quick Commands

### Before Committing
```bash
# Quick check (2-3 minutes)
make quick

# Full check (5-10 minutes)
make all
```

### Before Pushing
```bash
# Simulate full CI locally
make ci-simulation
```

### Specific Checks
```bash
make clippy-strict          # Strict clippy on core crates
make verify-patterns        # Check critical patterns
make feature-powerset       # Test feature combinations
make dead-code-cleanup      # Remove unused code
```

## 📋 Critical Patterns Checklist

### ✅ DO

```rust
// ✅ Use Profile::merge_with
let merged = Profile::merge_with(base, overlay);

// ✅ Engine::new with 2 arguments
let engine = Engine::new("demo".to_string(), config);

// ✅ BlackboxWriter::new without ?
let writer = BlackboxWriter::new(config);

// ✅ Use std::hint::black_box
std::hint::black_box(expensive_operation());

// ✅ Use workspace dependencies
[dependencies]
tokio = { workspace = true, features = ["macros"] }
```

### ❌ DON'T

```rust
// ❌ Don't use Profile::merge
let merged = Profile::merge(base, overlay);

// ❌ Don't use Engine::new with 1 argument
let engine = Engine::new(config);

// ❌ Don't use ? with BlackboxWriter::new
let writer = BlackboxWriter::new(config)?;

// ❌ Don't use criterion::black_box
criterion::black_box(expensive_operation());

// ❌ Don't specify versions directly
[dependencies]
tokio = { version = "1.35", features = ["macros"] }
```

## 🔧 Workspace Dependencies

Always use workspace dependencies for common crates:

```toml
[dependencies]
tokio = { workspace = true, features = ["macros"] }
futures = { workspace = true }
serde = { workspace = true, features = ["derive"] }
anyhow = { workspace = true }
thiserror = { workspace = true }
```

## 🎯 Core Crates (Strict Clippy)

These crates must pass `cargo clippy -- -D warnings`:

- flight-core
- flight-axis
- flight-bus
- flight-hid
- flight-ipc
- flight-service
- flight-simconnect
- flight-panels

## 🧪 Testing Features

Test your feature combinations:

```bash
# Test all feature combinations for your crate
cargo hack check -p your-crate --feature-powerset

# Test specific features
cargo check -p your-crate --features "feature1,feature2"
cargo check -p your-crate --no-default-features
```

## 🐛 Common Issues

### Issue: "Profile::merge not found"
**Fix**: Use `Profile::merge_with` instead

### Issue: "Engine::new expects 2 arguments"
**Fix**: Pass both name and config: `Engine::new("name".to_string(), config)`

### Issue: "BlackboxWriter::new returns T not Result"
**Fix**: Remove the `?` operator: `let writer = BlackboxWriter::new(config);`

### Issue: "criterion::black_box is deprecated"
**Fix**: Use `std::hint::black_box` instead

### Issue: "Feature X not enabled"
**Fix**: Check that workspace dependencies are used and features are properly gated

## 📊 CI Status

Check these before pushing:

- [ ] `make quick` passes locally
- [ ] All tests pass: `cargo test --workspace`
- [ ] No clippy warnings in core crates
- [ ] Critical patterns verified
- [ ] Feature combinations tested (if adding features)

## 🔗 Resources

- Full guide: `docs/regression-prevention.md`
- Makefile targets: `make help`
- CI workflow: `.github/workflows/ci.yml`
- Regression script: `scripts/regression_prevention.rs`

## 💡 Tips

1. **Run `make quick` frequently** during development
2. **Use workspace dependencies** for all common crates
3. **Test feature combinations** when adding new features
4. **Check CI logs** if builds fail - they show exactly what's wrong
5. **Update regression checks** when fixing new bugs

## 🆘 Getting Help

If regression checks fail:

1. Read the error message carefully
2. Check this quick reference
3. Review the full guide: `docs/regression-prevention.md`
4. Check recent commits for similar fixes
5. Ask the team in #dev-help

---

**Remember**: These checks exist to help you catch issues early, before they reach CI or production!
