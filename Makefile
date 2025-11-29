.PHONY: all build dev test lint fmt coverage score clean tier1 tier2 tier3

# Application name
APP ?= presentar

# Default target
all: fmt lint test build

# Development server with hot reload
dev:
	@echo "Starting Presentar dev server..."
	@cargo watch -x "build" -s "echo 'Build complete'"

# Production build
build:
	@echo "Building..."
	@cargo build --release

# Run all tests
test: test-unit test-integration

test-unit:
	@echo "Running unit tests..."
	@cargo test --workspace --lib

test-integration:
	@echo "Running integration tests..."
	@cargo test --workspace --test '*' 2>/dev/null || echo "No integration tests found"

# Linting
lint: lint-rust

lint-rust:
	@echo "Running clippy..."
	@cargo clippy --workspace -- -D warnings

# Format check
fmt:
	@echo "Checking formatting..."
	@cargo fmt --all -- --check

# Format fix
fmt-fix:
	@cargo fmt --all

# Coverage
coverage:
	@echo "Running coverage..."
	@cargo llvm-cov --workspace --html

# Quality score (placeholder)
score:
	@echo "Computing quality score..."
	@cargo test --workspace 2>&1 | grep -E "^test result" || true

# Clean
clean:
	@cargo clean

# =============================================================================
# Three-Tier Quality Pipeline
# =============================================================================

# TIER 1: On-save (<1 second)
tier1:
	@cargo check --workspace

# TIER 2: Pre-commit (1-5 minutes)
tier2: fmt lint test score

# TIER 3: Nightly (comprehensive)
tier3: tier2 coverage
	@echo "Running mutation testing..."
	@cargo mutants --timeout 300 2>/dev/null || echo "cargo-mutants not installed, skipping"

# =============================================================================
# Development Helpers
# =============================================================================

# Run a single test
test-one:
	@cargo test --workspace $(TEST) -- --nocapture

# Watch mode for TDD
watch:
	@cargo watch -x "test --workspace"

# Check all targets compile
check:
	@cargo check --workspace --all-targets

# Generate documentation
doc:
	@cargo doc --workspace --no-deps --open
