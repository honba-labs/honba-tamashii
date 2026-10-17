.PHONY: develop test build wheel clean

develop:
    maturin develop

test:
    pytest -q

build:
    maturin develop

wheel:
    maturin build --release

clean:
    rm -rf target dist build *.egg-info .pytest_cache
    find . -name pycache -type d -exec rm -rf {} +
