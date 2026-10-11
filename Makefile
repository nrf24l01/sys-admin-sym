.DEFAULT_GOAL := check
.NOTPARALLEL:

CARGO ?= cargo
PYTHON ?= python3
CARGO_SCOPE ?= --workspace
TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
GAME ?= $(TARGET_DIR)/debug/cloud-provider-sim
CLIENT ?= $(TARGET_DIR)/debug/game-ssh
ASSETS ?= assets
SMOKE_TIMEOUT ?= 30
CHECK_LOGS ?= $(TARGET_DIR)/check-logs

.PHONY: check check-core test fmt-check diff-check lint test-rust test-tools test-hosting build smoke smoke-built run run-release fmt help

# Cargo parallelizes internally; serialize these checks to avoid Cargo lock contention.
check: check-core smoke

test: check

check-core: fmt-check diff-check lint test-rust test-tools build

fmt-check:
	$(CARGO) fmt --all -- --check

diff-check:
	git diff --check

lint:
	$(CARGO) clippy $(CARGO_SCOPE) --all-targets --locked -- -D warnings

test-rust:
	$(CARGO) test $(CARGO_SCOPE) --locked

test-tools:
	$(PYTHON) -m unittest discover -s tools -p 'test_*.py'

test-hosting:
	$(CARGO) test -p cloud-provider-sim --test hosting --locked
	$(CARGO) test -p cloud-provider-game --locked ui::hosting

build:
	$(CARGO) build $(CARGO_SCOPE) --locked

smoke: build smoke-built

smoke-built:
	$(PYTHON) tools/smoke_game.py --game "$(GAME)" --client "$(CLIENT)" --assets "$(ASSETS)" --timeout "$(SMOKE_TIMEOUT)" --logs "$(CHECK_LOGS)"

# Every Make-driven game launch runs the complete suite first.
run: check
	"$(GAME)"

run-release: check
	$(CARGO) run --release --locked -p cloud-provider-game --bin cloud-provider-sim

fmt:
	$(CARGO) fmt --all

help:
	@printf '%s\n' \
	  'make / make check / make test  Run all checks, including Linux game startup' \
	  'make run                     Run all checks, then launch the development game' \
	  'make run-release             Run all checks, then launch the release game' \
	  'make check-core              Run checks without the Linux graphical smoke test' \
	  'make test-hosting            Run hosting simulation and UI regression tests' \
	  'make smoke                   Build and check the real game and terminal client' \
	  'make fmt                     Apply Rust formatting' \
	  'Targets: fmt-check diff-check lint test-rust test-tools build'
