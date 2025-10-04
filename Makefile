# OpenFlight Regression Prevention Makefile
# 
# This Makefile provides convenient targets for running regression prevention
# measures locally before pushing to CI.

.PHONY: all check-deps feature-powerset clippy-strict dead-code-cleanup verify-patterns clean

# Default target - run all regression prevention checks
all: check-deps feature-powerset clippy-strict verify-patterns

# Check that required tools are installed
check-deps:
	@echo "🔍 Checking required dependencies..."
	@command -v cargo >/dev/null 2>&1 || { echo "❌ cargo is required but not installed"; exit 1; }
	@command -v git >/dev/null 2>&1 || { echo "❌ git is required but not installed"; exit 1; }
	@cargo hack --version >/dev/null 2>&1 || { echo "📦 Installing cargo-hack..."; cargo install cargo-hack; }
	@echo "✅ All dependencies available"

# Run feature powerset testing
feature-powerset: check-deps
	@echo "🔍 Running feature powerset testing..."
	cargo hack check --workspace --feature-powerset --depth 2
	@echo "✅ Feature powerset testing passed"

# Run strict clippy checks on core crates
clippy-strict:
	@echo "🔍 Running strict clippy checks on core crates..."
	@for crate in flight-core flight-axis flight-bus flight-hid flight-ipc flight-service flight-simconnect flight-panels; do \
		echo "  Checking $$crate..."; \
		cargo clippy -p $$crate -- -D warnings || exit 1; \
	done
	@echo "✅ All core crates pass strict clippy checks"

# Clean up dead code and imports
dead-code-cleanup:
	@echo "🔍 Running dead code cleanup..."
	cargo fix --workspace --allow-dirty
	@echo "✅ Dead code cleanup completed"

# Verify critical patterns are fixed
verify-patterns:
	@echo "🔍 Verifying critical patterns are fixed..."
	
	@echo "  Checking Profile::merge usage..."
	@if git grep -n "Profile::merge(" | grep -v "Profile::merge_with"; then \
		echo "❌ Found Profile::merge( calls - should be Profile::merge_with"; \
		exit 1; \
	fi
	
	@echo "  Checking BlackboxWriter::new usage..."
	@if git grep -n "BlackboxWriter::new.*?"; then \
		echo "❌ Found BlackboxWriter::new with ? operator"; \
		exit 1; \
	fi
	
	@echo "  Checking Engine::new signature..."
	@if git grep -n "Engine::new(" | grep -v ","; then \
		echo "❌ Found Engine::new with incorrect signature"; \
		exit 1; \
	fi
	
	@echo "  Checking criterion::black_box usage..."
	@if git grep -n "criterion::black_box"; then \
		echo "❌ Found criterion::black_box - should be std::hint::black_box"; \
		exit 1; \
	fi
	
	@echo "  Checking workspace dependency alignment..."
	@if grep -r "tokio.*=" crates/*/Cargo.toml | grep -v "workspace = true" | grep -v "features\|optional"; then \
		echo "❌ Found non-workspace tokio dependencies"; \
		exit 1; \
	fi
	
	@if grep -r "futures.*=" crates/*/Cargo.toml | grep -v "workspace = true" | grep -v "features\|optional"; then \
		echo "❌ Found non-workspace futures dependencies"; \
		exit 1; \
	fi
	
	@echo "✅ All critical patterns verified"

# Verify workspace dependency alignment
check-workspace-deps:
	@echo "🔍 Checking workspace dependency alignment..."
	@echo "Tokio versions:"
	@grep -r "tokio.*=" Cargo.toml crates/*/Cargo.toml | grep version || true
	@echo "Futures versions:"
	@grep -r "futures.*=" Cargo.toml crates/*/Cargo.toml | grep version || true
	@echo "Tonic versions:"
	@grep -r "tonic.*=" Cargo.toml crates/*/Cargo.toml | grep version || true

# Clean build artifacts
clean:
	cargo clean

# Quick check - faster subset for development
quick: clippy-strict verify-patterns
	@echo "✅ Quick regression checks passed"

# Full CI simulation
ci-simulation: all
	@echo "🔍 Running full CI simulation..."
	cargo test --workspace
	cargo build --workspace --release
	@echo "✅ CI simulation completed successfully"

# Help target
help:
	@echo "OpenFlight Regression Prevention Targets:"
	@echo ""
	@echo "  all                 - Run all regression prevention checks (default)"
	@echo "  check-deps          - Check that required tools are installed"
	@echo "  feature-powerset    - Run feature powerset testing"
	@echo "  clippy-strict       - Run strict clippy checks on core crates"
	@echo "  dead-code-cleanup   - Clean up dead code and imports"
	@echo "  verify-patterns     - Verify critical patterns are fixed"
	@echo "  check-workspace-deps- Check workspace dependency alignment"
	@echo "  quick               - Run quick checks (clippy + patterns)"
	@echo "  ci-simulation       - Run full CI simulation"
	@echo "  clean               - Clean build artifacts"
	@echo "  help                - Show this help message"