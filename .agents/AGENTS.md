# `honba-tamashii` Agent Guidelines & Architecture Invariants

**Subsystem**: Soul (Python FFI Bridge & Wheel Compiler)
**Language**: Rust (PyO3) + Python
**Primary Responsibility**: Compiles the `honba` PyPI wheel, Apache Arrow zero-copy memory bridge, and smart GIL management.

---

## 1. Role & Architectural Hints
* `honba-tamashii` is the bridge between the high-performance Rust core and the Python-first developer plane:
  1. **PyO3 FFI Layer**: Exposes canonical Rust types (`Price`, `Quantity`, `Bar`, `Tick`, `Order`, `Decision`) to Python.
  2. **Zero-Copy Arrow Memory Bridge**: High-volume tick batches and historical bars transfer between Rust and Python via Apache Arrow RecordBatches / PyCapsule protocol with zero deserialization overhead.
  3. **Smart GIL Management**: Long-running Rust simulation steps, indicator loops, and network I/O release the Python GIL (`py.allow_threads`). The GIL is acquired strictly when invoking Python user strategy callbacks (`on_bar`, `on_tick`, `on_order_event`).
  4. **Single Source of Truth**: Generates Python type stubs (`.pyi`) from Rust types via `make codegen`.
* **Idea Inheritance**:
  - **NautilusTrader**: PyO3/Cython zero-copy memory bridge and fast event dispatching.

---

## 2. Coding Guidelines (DDD & TDD)
* **Zero Floats Policy**: Decimals across all FFI boundaries (`rust_decimal::Decimal` <-> `decimal.Decimal`).
* **TDD & Parallel Test Directory Rule (MANDATORY)**:
  - All tests must live in `tests/` parallel to `src/`.
  - **STRICTLY FORBIDDEN**: In-file `#[test]` inside `src/`.

---

## 3. Retargetability & Portability
* Multi-platform wheels built via `maturin` targeting `linux-x86_64` and `linux-aarch64`.

---

## 4. Development & CI Workflow
* **Build Wheel**:
  ```bash
  maturin develop
  ```
* **Linting & Testing**:
  ```bash
  cargo clippy -- -D warnings
  pytest -q
  ```
