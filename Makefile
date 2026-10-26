.PHONY: default build build-dev build-prod build-release build-debug dev prod release debug develop test wheel check check-dev check-prod clean pyi stubtest invariants mutation

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

pyi:
	python3 -c "from honba._honba import codegen_render; _, content = codegen_render('native_pyi'); open('_honba.pyi', 'w').write(content)"

stubtest:
	python3 scripts/check_stub_compatibility.py

invariants:
	pytest -q tests/test_invariants.py

mutation:
	python3 -m mutmut run

clean:
	cargo clean
	rm -rf target dist build *.egg-info .pytest_cache .mutmut-cache
	find . -name __pycache__ -type d -exec rm -rf {} +
