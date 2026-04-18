# tui-dispatch Makefile
# Convenience targets for build, test, lint, and development

.PHONY: all build check test test-feature-matrix fmt clippy clean help verify release lint fmt-check doc docs-serve tag

# Default target
all: build

# Build the project (debug)
build:
	cargo build

# Build release version
release:
	cargo build --release

lint: fmt-check check clippy

# Check compilation without building
check:
	cargo check --all --all-features

# Run all tests
test:
	cargo test --all --all-features

# Run tui-dispatch-core contract tests across every supported feature combo
# (PR1 / RUNTIME_PR_PLAN.md — keep the runtime contract validated under the
# feature axes consumers actually build with, not only `--all-features`).
# NOTE: the same combo list is mirrored in .github/workflows/ci.yml under the
# `test-feature-matrix` job — keep the two in sync when adding/removing combos.
test-feature-matrix:
	@set -e; for feats in "" "tasks" "subscriptions" "debug" \
		"tasks,subscriptions" "debug,tasks" "debug,subscriptions" \
		"debug,tasks,subscriptions"; do \
		echo "==> tui-dispatch-core contract tests (features='$$feats')"; \
		if [ -z "$$feats" ]; then \
			cargo test -p tui-dispatch-core --tests --no-default-features; \
		else \
			cargo test -p tui-dispatch-core --tests --no-default-features --features "$$feats"; \
		fi; \
	done

# Format code
fmt:
	cargo fmt --all

# Check formatting (CI-friendly)
fmt-check:
	cargo fmt --all -- --check

# Run clippy linter
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Full verification (for CI/pre-commit)
verify: fmt-check check clippy test doc

# Build documentation (library crates only, excludes examples)
doc:
	cargo doc --no-deps -p tui-dispatch -p tui-dispatch-core -p tui-dispatch-macros

# Serve Astro/Starlight documentation locally
docs-serve:
	npm --prefix docs run dev

# Create and push a release tag (runs full verification first)
tag:
	@if [ -n "$$(git status --porcelain)" ]; then \
		echo "Error: Working tree is dirty. Commit or stash changes first."; \
		exit 1; \
	fi
	@if [ "$$(git branch --show-current)" != "main" ]; then \
		echo "Error: Not on main branch."; \
		exit 1; \
	fi
	@VERSION=$$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "tui-dispatch") | .version'); \
	if ! grep -q "^## \[$$VERSION\]" CHANGELOG.md; then \
		echo "Error: CHANGELOG.md has no entry for version $$VERSION"; \
		echo "Add a section like: ## [$$VERSION] - $$(date +%Y-%m-%d)"; \
		exit 1; \
	fi
	@echo "Running full verification..."
	@$(MAKE) verify
	@VERSION=$$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "tui-dispatch") | .version'); \
	if git rev-parse "v$$VERSION" >/dev/null 2>&1; then \
		echo "Error: Tag v$$VERSION already exists."; \
		exit 1; \
	fi; \
	echo "Creating tag v$$VERSION..."; \
	git tag "v$$VERSION" && \
	echo "Pushing tag v$$VERSION to origin..."; \
	git push origin "v$$VERSION" && \
	echo "Done! Release v$$VERSION has been tagged and pushed."

# Clean build artifacts
clean:
	cargo clean

# Show help
help:
	@echo "tui-dispatch - Makefile targets:"
	@echo ""
	@echo "  make build       - Build debug"
	@echo "  make release     - Build release"
	@echo "  make check       - Check compilation"
	@echo "  make test        - Run tests"
	@echo "  make test-feature-matrix - Run core contract tests across feature combos"
	@echo "  make fmt         - Format code"
	@echo "  make fmt-check   - Check code formatting"
	@echo "  make clippy      - Run linter"
	@echo "  make lint        - Run fmt-check, check, and clippy"
	@echo "  make verify      - Run all checks (CI)"
	@echo "  make doc         - Build docs (library crates only)"
	@echo "  make docs-serve  - Serve Astro docs locally"
	@echo "  make tag         - Create release tag (runs verify first)"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make help        - Show this help"
