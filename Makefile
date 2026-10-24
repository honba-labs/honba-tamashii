.PHONY: default build build-dev build-prod build-release build-debug dev prod release debug develop test wheel check check-dev check-prod clean

default: dev
build: dev

dev: build-dev
build-dev:
	maturin develop

develop: build-dev

prod: build-prod
build-prod:
	maturin build --release

release: build-release
build-release:
	maturin build --release

wheel: build-prod

# Debug build is explicit and not run unless asked
debug: build-debug
build-debug:
	cargo build --workspace

check: check-dev
check-dev:
	cargo check --workspace --profile dev

check-prod:
	cargo check --workspace --profile prod

test:
	pytest -q

clean:
	cargo clean
	rm -rf target dist build *.egg-info .pytest_cache
	find . -name __pycache__ -type d -exec rm -rf {} +
