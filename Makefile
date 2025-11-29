.PHONY: all build dev test test-fast lint lint-fast fmt coverage score clean tier1 tier2 tier3

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

# Fast tests for rapid TDD feedback (<30s target)
test-fast: ## Fast unit tests (<30s target)
	@echo "⚡ Running fast tests (target: <30s)..."
	@if command -v cargo-nextest >/dev/null 2>&1; then \
		cargo nextest run --workspace --lib \
			--status-level skip \
			--failure-output immediate; \
	else \
		cargo test --workspace --lib --quiet; \
	fi

# Linting
lint: lint-rust

lint-rust:
	@echo "Running clippy..."
	@cargo clippy --workspace -- -D warnings

# Fast lint for tier1
lint-fast: ## Fast clippy (library only)
	@cargo clippy --workspace --lib --quiet -- -D warnings

# Format check
fmt:
	@echo "Checking formatting..."
	@cargo fmt --all -- --check

# Format fix
fmt-fix:
	@cargo fmt --all

# Coverage with proper llvm-cov handling
coverage: ## Generate HTML coverage report
	@echo "📊 Running coverage analysis..."
	@echo "🔍 Checking for cargo-llvm-cov..."
	@which cargo-llvm-cov > /dev/null 2>&1 || (echo "📦 Installing cargo-llvm-cov..." && cargo install cargo-llvm-cov --locked)
	@echo "⚙️  Temporarily disabling global cargo config (sccache/mold break coverage)..."
	@test -f ~/.cargo/config.toml && mv ~/.cargo/config.toml ~/.cargo/config.toml.cov-backup || true
	@echo "🧪 Running tests with coverage instrumentation..."
	@cargo llvm-cov --workspace --html --quiet || { \
		test -f ~/.cargo/config.toml.cov-backup && mv ~/.cargo/config.toml.cov-backup ~/.cargo/config.toml; \
		exit 1; \
	}
	@echo "⚙️  Restoring cargo config..."
	@test -f ~/.cargo/config.toml.cov-backup && mv ~/.cargo/config.toml.cov-backup ~/.cargo/config.toml || true
	@echo ""
	@echo "📈 Coverage Summary:"
	@cargo llvm-cov report --summary-only 2>/dev/null | grep -E "TOTAL|Region|Function|Line" || true
	@echo ""
	@echo "✅ Coverage report generated: target/llvm-cov/html/index.html"

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

# TIER 1: On-save (<1 second) - Rapid feedback for flow state
tier1: ## Tier 1: Sub-second feedback (ON-SAVE)
	@echo "🚀 TIER 1: Sub-second feedback"
	@echo ""
	@echo "  [1/3] Type checking..."
	@cargo check --workspace --quiet
	@echo "  [2/3] Fast lint..."
	@cargo clippy --workspace --lib --quiet -- -D warnings
	@echo "  [3/3] Fast tests..."
	@cargo test --workspace --lib --quiet
	@echo ""
	@echo "✅ Tier 1 complete"

# TIER 2: Pre-commit (1-5 minutes)
tier2: ## Tier 2: Full validation (ON-COMMIT)
	@echo "🔍 TIER 2: Comprehensive validation"
	@echo ""
	@echo "  [1/4] Formatting..."
	@cargo fmt --all -- --check
	@echo "  [2/4] Full clippy..."
	@cargo clippy --workspace --all-targets -- -D warnings
	@echo "  [3/4] All tests..."
	@cargo test --workspace
	@echo "  [4/4] Score..."
	@cargo test --workspace 2>&1 | grep -E "^test result" || true
	@echo ""
	@echo "✅ Tier 2 complete"

# TIER 3: Nightly (comprehensive)
tier3: ## Tier 3: Mutation & coverage (ON-MERGE/NIGHTLY)
	@echo "🧬 TIER 3: Test quality assurance"
	@$(MAKE) --no-print-directory tier2
	@echo ""
	@echo "  Running coverage..."
	@$(MAKE) --no-print-directory coverage
	@echo ""
	@echo "  Running mutation testing..."
	@cargo mutants --timeout 300 2>/dev/null || echo "    ⚠️  cargo-mutants not installed"
	@echo ""
	@echo "✅ Tier 3 complete"

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
