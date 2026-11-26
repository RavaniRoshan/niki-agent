MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
# Niki Agent is a single package under `code/`; nothing else is built.
LIB_PACKAGE_DIRS := code
PACKAGE_DIRS := code

python_version = 3.12

.PHONY: help lock no-cache lock-bump lock-check lint format bench-all

LOCK_ARGS := $(if $(filter no-cache,$(MAKECMDGOALS)),--no-cache,)

BENCH_PACKAGES := code

.DEFAULT_GOAL := help

help: ## Show this help message
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

lock: ## Update all lockfiles (append 'no-cache' to bypass uv's cache)
	@set -e; \
	$(foreach pkg,$(PACKAGE_DIRS), \
		echo "🔒 Locking $(pkg)"; \
		uv lock --directory '$(pkg)' --python '$(call python_version,$(pkg))' $(LOCK_ARGS); \
	)
	@echo "✅ All lockfiles updated!"

no-cache:
	@:

lock-bump: ## Bump a dependency across all lockfiles (usage: make lock-bump DEP=requests)
	@if [ -z "$(DEP)" ]; then \
		echo "Usage: make lock-bump DEP=<package>"; \
		exit 1; \
	fi
	@set -e; \
	$(foreach pkg,$(PACKAGE_DIRS), \
		echo "⬆️  Bumping $(DEP) in $(pkg)"; \
		uv lock --directory '$(pkg)' --python '$(call python_version,$(pkg))' -P '$(DEP)'; \
	)
	@echo "✅ Bumped $(DEP) across all lockfiles!"

lock-check: ## Check all lockfiles are up-to-date
	@set -e; \
	$(foreach pkg,$(PACKAGE_DIRS), \
		echo "🔍 Checking $(pkg)"; \
		uv lock --check --directory '$(pkg)' --python '$(call python_version,$(pkg))'; \
	)
	@echo "✅ All lockfiles are up-to-date!"

lint: ## Lint all packages
	@set -e; \
	for dir in $(LIB_PACKAGE_DIRS); do \
		echo "🔍 Linting $$dir"; \
		$(MAKE) -C "$$dir" lint; \
	done
	@echo "✅ All packages linted!"

format: ## Format all packages
	@set -e; \
	for dir in $(LIB_PACKAGE_DIRS); do \
		echo "🎨 Formatting $$dir"; \
		$(MAKE) -C "$$dir" format; \
	done
	@echo "✅ All packages formatted!"

bench-all: ## Run benchmarks across all benched packages
	@set -e; \
	for pkg in $(BENCH_PACKAGES); do \
		echo "--> bench: $$pkg"; \
		$(MAKE) -C $$pkg bench; \
	done
