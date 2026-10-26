//! The native surface of `honba._honba`, described once and rendered as a
//! strict `.pyi` stub.
//!
//! The description is data, so the stub is generated, never hand-edited. A
//! test in the extension crate checks that every name declared here exists in
//! the built module, and that every public name in the module is declared
//! here: the stub cannot drift from the implementation in either direction.

/// One method of a native class.
pub struct Method {
    /// The method name.
    pub name: &'static str,
    /// The full `def` line, including the return annotation.
    pub signature: &'static str,
}

/// One attribute of a native class.
pub struct Attribute {
    /// The attribute name.
    pub name: &'static str,
    /// The attribute's type annotation.
    pub annotation: &'static str,
}

/// One native class.
pub struct Class {
    /// The class name.
    pub name: &'static str,
    /// The class docstring.
    pub doc: &'static str,
    /// The instance attributes.
    pub attributes: &'static [Attribute],
    /// The methods.
    pub methods: &'static [Method],
}

/// One module-level function.
pub struct Function {
    /// The function name.
    pub name: &'static str,
    /// The full `def` line, including the return annotation.
    pub signature: &'static str,
}

/// The module constants.
pub const CONSTANTS: &[(&str, &str)] = &[
    ("SCHEMA_VERSION", "int"),
    ("API_VERSION", "str"),
    ("STRATEGY_API_VERSION", "str"),
];

/// The native classes.
pub const CLASSES: &[Class] = &[
    Class {
        name: "Price",
        doc: "An exact decimal price.",
        attributes: &[],
        methods: &[
            Method {
                name: "to_decimal",
                signature: "def to_decimal(self) -> decimal.Decimal: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
            Method {
                name: "is_positive",
                signature: "def is_positive(self) -> bool: ...",
            },
            Method {
                name: "is_tick_aligned",
                signature: "def is_tick_aligned(self, tick_size: decimal.Decimal) -> bool: ...",
            },
        ],
    },
    Class {
        name: "Quantity",
        doc: "An exact decimal quantity.",
        attributes: &[],
        methods: &[
            Method {
                name: "to_decimal",
                signature: "def to_decimal(self) -> decimal.Decimal: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
            Method {
                name: "is_positive",
                signature: "def is_positive(self) -> bool: ...",
            },
            Method {
                name: "is_lot_multiple",
                signature: "def is_lot_multiple(self, lot_size: int) -> bool: ...",
            },
        ],
    },
    Class {
        name: "Money",
        doc: "An exact monetary amount in a currency.",
        attributes: &[Attribute {
            name: "currency",
            annotation: "str",
        }],
        methods: &[
            Method {
                name: "to_decimal",
                signature: "def to_decimal(self) -> decimal.Decimal: ...",
            },
            Method {
                name: "to_major",
                signature: "def to_major(self) -> decimal.Decimal: ...",
            },
            Method {
                name: "to_minor",
                signature: "def to_minor(self) -> decimal.Decimal: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "MinorUnit",
        doc: "The display names of a currency's minor unit.",
        attributes: &[
            Attribute {
                name: "singular",
                annotation: "str",
            },
            Attribute {
                name: "plural",
                annotation: "str",
            },
        ],
        methods: &[Method {
            name: "of_currency",
            signature: "def of_currency(currency: str) -> MinorUnit: ...",
        }],
    },
    Class {
        name: "InstrumentId",
        doc: "A canonical instrument identifier (`NSE:RELIANCE`).",
        attributes: &[
            Attribute {
                name: "symbol",
                annotation: "str",
            },
            Attribute {
                name: "exchange",
                annotation: "str",
            },
            Attribute {
                name: "token",
                annotation: "str | None",
            },
            Attribute {
                name: "isin",
                annotation: "str | None",
            },
        ],
        methods: &[
            Method {
                name: "from_string",
                signature: "def from_string(text: str) -> InstrumentId: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "PriceBand",
        doc: "A scrip-wise circuit band.",
        attributes: &[
            Attribute {
                name: "lower",
                annotation: "Price",
            },
            Attribute {
                name: "upper",
                annotation: "Price",
            },
            Attribute {
                name: "reference_price",
                annotation: "Price",
            },
            Attribute {
                name: "band_pct",
                annotation: "decimal.Decimal | None",
            },
        ],
        methods: &[
            Method {
                name: "contains",
                signature: "def contains(self, price: Price) -> bool: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "DepthLevel",
        doc: "One level of the order book.",
        attributes: &[
            Attribute {
                name: "price",
                annotation: "Price",
            },
            Attribute {
                name: "quantity",
                annotation: "Quantity",
            },
            Attribute {
                name: "orders_count",
                annotation: "int | None",
            },
        ],
        methods: &[],
    },
    Class {
        name: "Bar",
        doc: "One OHLCV bar.",
        attributes: &[
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "open",
                annotation: "Price",
            },
            Attribute {
                name: "high",
                annotation: "Price",
            },
            Attribute {
                name: "low",
                annotation: "Price",
            },
            Attribute {
                name: "close",
                annotation: "Price",
            },
            Attribute {
                name: "volume",
                annotation: "Quantity",
            },
            Attribute {
                name: "timestamp",
                annotation: "datetime.datetime",
            },
        ],
        methods: &[Method {
            name: "to_json",
            signature: "def to_json(self) -> str: ...",
        }],
    },
    Class {
        name: "Tick",
        doc: "A market-data tick: last price, quote or depth.",
        attributes: &[
            Attribute {
                name: "kind",
                annotation: "str",
            },
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "last_price",
                annotation: "Price | None",
            },
            Attribute {
                name: "bid",
                annotation: "Price | None",
            },
            Attribute {
                name: "ask",
                annotation: "Price | None",
            },
            Attribute {
                name: "volume",
                annotation: "Quantity | None",
            },
            Attribute {
                name: "exchange_ts",
                annotation: "datetime.datetime",
            },
            Attribute {
                name: "receive_ts",
                annotation: "datetime.datetime",
            },
            Attribute {
                name: "band",
                annotation: "PriceBand | None",
            },
        ],
        methods: &[
            Method {
                name: "from_json",
                signature: "def from_json(payload: str) -> Tick: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "OrderSide",
        doc: "Buy or sell.",
        attributes: &[
            Attribute {
                name: "BUY",
                annotation: "OrderSide",
            },
            Attribute {
                name: "SELL",
                annotation: "OrderSide",
            },
        ],
        methods: &[],
    },
    Class {
        name: "OrderType",
        doc: "The order type.",
        attributes: &[
            Attribute {
                name: "MARKET",
                annotation: "OrderType",
            },
            Attribute {
                name: "LIMIT",
                annotation: "OrderType",
            },
            Attribute {
                name: "STOP_LOSS",
                annotation: "OrderType",
            },
            Attribute {
                name: "STOP_LIMIT",
                annotation: "OrderType",
            },
        ],
        methods: &[],
    },
    Class {
        name: "TimeInForce",
        doc: "The order's time in force.",
        attributes: &[
            Attribute {
                name: "DAY",
                annotation: "TimeInForce",
            },
            Attribute {
                name: "IOC",
                annotation: "TimeInForce",
            },
            Attribute {
                name: "GTC",
                annotation: "TimeInForce",
            },
        ],
        methods: &[],
    },
    Class {
        name: "OrderStatus",
        doc: "The lifecycle state of an order.",
        attributes: &[
            Attribute {
                name: "PENDING_NEW",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "NEW",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "PARTIALLY_FILLED",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "FILLED",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "CANCELLED",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "REJECTED",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "EXPIRED",
                annotation: "OrderStatus",
            },
        ],
        methods: &[Method {
            name: "is_terminal",
            signature: "def is_terminal(self) -> bool: ...",
        }],
    },
    Class {
        name: "OrderRequest",
        doc: "An order to be sent to the exchange.",
        attributes: &[
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "side",
                annotation: "OrderSide",
            },
            Attribute {
                name: "order_type",
                annotation: "OrderType",
            },
            Attribute {
                name: "quantity",
                annotation: "Quantity",
            },
            Attribute {
                name: "price",
                annotation: "Price | None",
            },
            Attribute {
                name: "trigger_price",
                annotation: "Price | None",
            },
            Attribute {
                name: "tif",
                annotation: "TimeInForce",
            },
            Attribute {
                name: "algo_id",
                annotation: "str",
            },
            Attribute {
                name: "client_order_id",
                annotation: "str",
            },
        ],
        methods: &[Method {
            name: "to_json",
            signature: "def to_json(self) -> str: ...",
        }],
    },
    Class {
        name: "OrderAck",
        doc: "The exchange's acknowledgement of an order.",
        attributes: &[
            Attribute {
                name: "order_id",
                annotation: "str",
            },
            Attribute {
                name: "client_order_id",
                annotation: "str",
            },
            Attribute {
                name: "status",
                annotation: "OrderStatus",
            },
            Attribute {
                name: "timestamp",
                annotation: "datetime.datetime",
            },
        ],
        methods: &[Method {
            name: "to_json",
            signature: "def to_json(self) -> str: ...",
        }],
    },
    Class {
        name: "FillReport",
        doc: "A trade execution.",
        attributes: &[
            Attribute {
                name: "fill_id",
                annotation: "str",
            },
            Attribute {
                name: "order_id",
                annotation: "str",
            },
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "side",
                annotation: "OrderSide",
            },
            Attribute {
                name: "price",
                annotation: "Price",
            },
            Attribute {
                name: "quantity",
                annotation: "Quantity",
            },
            Attribute {
                name: "algo_id",
                annotation: "str",
            },
            Attribute {
                name: "timestamp",
                annotation: "datetime.datetime",
            },
        ],
        methods: &[Method {
            name: "to_json",
            signature: "def to_json(self) -> str: ...",
        }],
    },
    Class {
        name: "OrderIntent",
        doc: "One order the strategy asked for.",
        attributes: &[
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "side",
                annotation: "str",
            },
            Attribute {
                name: "quantity",
                annotation: "Quantity",
            },
            Attribute {
                name: "order_type",
                annotation: "str",
            },
            Attribute {
                name: "price",
                annotation: "Price | None",
            },
            Attribute {
                name: "algo_id",
                annotation: "str",
            },
        ],
        methods: &[],
    },
    Class {
        name: "RiskLimits",
        doc: "The limits a run trades under.",
        attributes: &[
            Attribute {
                name: "max_notional_per_order",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "max_quantity_per_order",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "max_daily_loss",
                annotation: "decimal.Decimal | None",
            },
        ],
        methods: &[
            Method {
                name: "from_dict",
                signature: "def from_dict(config: dict[str, Any]) -> RiskLimits: ...",
            },
            Method {
                name: "is_empty",
                signature: "def is_empty(self) -> bool: ...",
            },
            Method {
                name: "require_live",
                signature: "def require_live(self) -> None: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "OtrConfig",
        doc: "The order-to-trade ratio breaker's configuration.",
        attributes: &[Attribute {
            name: "max_otr",
            annotation: "decimal.Decimal",
        }],
        methods: &[Method {
            name: "to_json",
            signature: "def to_json(self) -> str: ...",
        }],
    },
    Class {
        name: "RiskRefusal",
        doc: "A refusal by the risk gate.",
        attributes: &[
            Attribute {
                name: "rule",
                annotation: "str",
            },
            Attribute {
                name: "reason",
                annotation: "str",
            },
        ],
        methods: &[
            Method {
                name: "details",
                signature: "def details(self) -> dict[str, Any]: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "RiskRefusalError",
        doc: "Raised when the risk gate refuses an order.",
        attributes: &[
            Attribute {
                name: "rule",
                annotation: "str",
            },
            Attribute {
                name: "refusal",
                annotation: "str",
            },
        ],
        methods: &[],
    },
    Class {
        name: "RiskEngine",
        doc: "The pre-trade risk gate.",
        attributes: &[],
        methods: &[
            Method {
                name: "register_algo",
                signature: "def register_algo(self, algo_id: str) -> None: ...",
            },
            Method {
                name: "suspend_algo",
                signature: "def suspend_algo(self, algo_id: str, reason: str) -> None: ...",
            },
            Method {
                name: "reactivate_algo",
                signature: "def reactivate_algo(self, algo_id: str) -> None: ...",
            },
            Method {
                name: "unregister_algo",
                signature: "def unregister_algo(self, algo_id: str) -> None: ...",
            },
            Method {
                name: "is_algo_registered",
                signature: "def is_algo_registered(self, algo_id: str) -> bool: ...",
            },
            Method {
                name: "halt_global",
                signature: "def halt_global(self, reason: str, now: str | datetime.datetime | None = None) -> None: ...",
            },
            Method {
                name: "resume_global",
                signature: "def resume_global(self) -> None: ...",
            },
            Method {
                name: "halt_algo",
                signature: "def halt_algo(self, algo_id: str, reason: str, now: str | datetime.datetime | None = None) -> None: ...",
            },
            Method {
                name: "resume_algo",
                signature: "def resume_algo(self, algo_id: str) -> None: ...",
            },
            Method {
                name: "set_price_band",
                signature: "def set_price_band(self, instrument: InstrumentId, band: PriceBand) -> None: ...",
            },
            Method {
                name: "check_order",
                signature: "def check_order(self, order: OrderRequest, now: str | datetime.datetime | None = None) -> None: ...",
            },
            Method {
                name: "check_order_fast",
                signature: "def check_order_fast(self, algo_id: str, instrument: InstrumentId, quantity: decimal.Decimal, limit_price: decimal.Decimal | None = None, now: str | datetime.datetime | None = None) -> None: ...",
            },
            Method {
                name: "check_decision",
                signature: "def check_decision(self, decision: str, now: str | datetime.datetime | None = None) -> None: ...",
            },
            Method {
                name: "record_fill",
                signature: "def record_fill(self, fill: FillReport) -> None: ...",
            },
            Method {
                name: "record_cancellation",
                signature: "def record_cancellation(self, algo_id: str, now: str | datetime.datetime | None = None) -> None: ...",
            },
            Method {
                name: "otr_for_algo",
                signature: "def otr_for_algo(self, algo_id: str) -> decimal.Decimal | None: ...",
            },
            Method {
                name: "total_refusals",
                signature: "def total_refusals(self) -> int: ...",
            },
            Method {
                name: "refusals_for_rule",
                signature: "def refusals_for_rule(self, rule: str) -> int: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "RiskDecision",
        doc: "The result of one risk check.",
        attributes: &[
            Attribute {
                name: "approved",
                annotation: "bool",
            },
            Attribute {
                name: "code",
                annotation: "str",
            },
            Attribute {
                name: "rule",
                annotation: "str",
            },
            Attribute {
                name: "context",
                annotation: "str",
            },
        ],
        methods: &[],
    },
    Class {
        name: "RiskStage",
        doc: "A risk gate in front of an order flow.",
        attributes: &[Attribute {
            name: "engine",
            annotation: "RiskEngine",
        }],
        methods: &[
            Method {
                name: "check",
                signature: "def check(self, order: OrderRequest, now: str | datetime.datetime | None = None) -> RiskDecision: ...",
            },
            Method {
                name: "register_algo",
                signature: "def register_algo(self, algo_id: str) -> None: ...",
            },
        ],
    },
    Class {
        name: "TradingState",
        doc: "The trading state a session can be in.",
        attributes: &[
            Attribute {
                name: "ACTIVE",
                annotation: "TradingState",
            },
            Attribute {
                name: "REDUCING",
                annotation: "TradingState",
            },
            Attribute {
                name: "HALTED",
                annotation: "TradingState",
            },
        ],
        methods: &[],
    },
    Class {
        name: "Command",
        doc: "An operator command a live shell accepts.",
        attributes: &[
            Attribute {
                name: "kind",
                annotation: "str",
            },
            Attribute {
                name: "state",
                annotation: "TradingState | None",
            },
        ],
        methods: &[
            Method {
                name: "stop",
                signature: "def stop() -> Command: ...",
            },
            Method {
                name: "state",
                signature: "def state(state: TradingState) -> Command: ...",
            },
        ],
    },
    Class {
        name: "Metrics",
        doc: "What a run produced.",
        attributes: &[
            Attribute {
                name: "orders",
                annotation: "list[OrderIntent]",
            },
            Attribute {
                name: "final_cash",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "degraded",
                annotation: "bool",
            },
            Attribute {
                name: "errors",
                annotation: "list[str]",
            },
            Attribute {
                name: "refusals",
                annotation: "list[str]",
            },
        ],
        methods: &[],
    },
    Class {
        name: "PyContext",
        doc: "The context a strategy submits through.",
        attributes: &[
            Attribute {
                name: "orders",
                annotation: "list[OrderIntent]",
            },
            Attribute {
                name: "refusals",
                annotation: "list[str]",
            },
        ],
        methods: &[
            Method {
                name: "submit",
                signature: "def submit(self, instrument: InstrumentId | str, side: str, quantity: decimal.Decimal | int | str) -> str: ...",
            },
            Method {
                name: "submit_request",
                signature: "def submit_request(self, order: OrderRequest) -> str: ...",
            },
            Method {
                name: "close_all",
                signature: "def close_all(self, instrument: InstrumentId | str) -> str: ...",
            },
        ],
    },
    Class {
        name: "Runner",
        doc: "The strategy harness: runs a Python strategy over a synthetic price walk.",
        attributes: &[
            Attribute {
                name: "num_bars",
                annotation: "int",
            },
            Attribute {
                name: "instrument",
                annotation: "str",
            },
            Attribute {
                name: "starting_cash",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "algo_id",
                annotation: "str",
            },
        ],
        methods: &[Method {
            name: "run",
            signature: "def run(self, strategy: Any) -> Metrics: ...",
        }],
    },
    Class {
        name: "Sma",
        doc: "A simple moving average over exact decimals.",
        attributes: &[Attribute {
            name: "value",
            annotation: "decimal.Decimal | None",
        }],
        methods: &[Method {
            name: "update",
            signature: "def update(self, value: decimal.Decimal | int | str) -> decimal.Decimal | None: ...",
        }],
    },
    Class {
        name: "RustSmaCrossover",
        doc: "Rust-backed SMA crossover.",
        attributes: &[
            Attribute {
                name: "position",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "intent_count",
                annotation: "int",
            },
        ],
        methods: &[
            Method {
                name: "on_bar",
                signature: "def on_bar(self, bar: Bar) -> tuple[str, decimal.Decimal] | None: ...",
            },
            Method {
                name: "on_close",
                signature: "def on_close(self, close: decimal.Decimal | int | str) -> tuple[str, decimal.Decimal] | None: ...",
            },
            Method {
                name: "intents",
                signature: "def intents(self) -> list[tuple[str, decimal.Decimal]]: ...",
            },
        ],
    },
    Class {
        name: "FnoContract",
        doc: "Parsed Indian F&O contract details.",
        attributes: &[
            Attribute {
                name: "symbol",
                annotation: "str",
            },
            Attribute {
                name: "underlying",
                annotation: "str",
            },
            Attribute {
                name: "kind",
                annotation: "str",
            },
            Attribute {
                name: "expiry_year",
                annotation: "int",
            },
            Attribute {
                name: "expiry_month",
                annotation: "int",
            },
            Attribute {
                name: "expiry_day",
                annotation: "int | None",
            },
            Attribute {
                name: "is_weekly",
                annotation: "bool",
            },
            Attribute {
                name: "strike",
                annotation: "str | None",
            },
            Attribute {
                name: "option_type",
                annotation: "str | None",
            },
        ],
        methods: &[],
    },
    Class {
        name: "ArrowStream",
        doc: "Zero-copy Arrow PyCapsule stream.",
        attributes: &[
            Attribute {
                name: "num_rows",
                annotation: "int",
            },
            Attribute {
                name: "num_batches",
                annotation: "int",
            },
            Attribute {
                name: "column_names",
                annotation: "list[str]",
            },
        ],
        methods: &[
            Method {
                name: "__arrow_c_schema__",
                signature: "def __arrow_c_schema__(self) -> Any: ...",
            },
            Method {
                name: "__arrow_c_stream__",
                signature: "def __arrow_c_stream__(self, requested_schema: Any = None) -> Any: ...",
            },
            Method {
                name: "to_pyarrow",
                signature: "def to_pyarrow(self) -> Any: ...",
            },
            Method {
                name: "to_polars",
                signature: "def to_polars(self) -> Any: ...",
            },
        ],
    },
    Class {
        name: "Lakehouse",
        doc: "Parquet Lakehouse v2 reader and writer.",
        attributes: &[Attribute {
            name: "root",
            annotation: "str",
        }],
        methods: &[
            Method {
                name: "write_bars",
                signature: "def write_bars(self, timeframe: str, bars: list[Bar]) -> list[str]: ...",
            },
            Method {
                name: "write_ticks",
                signature: "def write_ticks(self, ticks: list[Tick]) -> list[str]: ...",
            },
            Method {
                name: "read_bars",
                signature: "def read_bars(self, instrument: InstrumentId | str, timeframe: str, start: str | datetime.datetime, end: str | datetime.datetime) -> list[Bar]: ...",
            },
            Method {
                name: "read_ticks",
                signature: "def read_ticks(self, instrument: InstrumentId | str, start: str | datetime.datetime, end: str | datetime.datetime) -> list[Tick]: ...",
            },
            Method {
                name: "load_bars_arrow",
                signature: "def load_bars_arrow(self, instrument: InstrumentId | str, timeframe: str, start: str | datetime.datetime, end: str | datetime.datetime) -> ArrowStream: ...",
            },
            Method {
                name: "load_ticks_arrow",
                signature: "def load_ticks_arrow(self, instrument: InstrumentId | str, start: str | datetime.datetime, end: str | datetime.datetime) -> ArrowStream: ...",
            },
        ],
    },
    Class {
        name: "BhavRecord",
        doc: "One scrip's NSE Bhavcopy record.",
        attributes: &[
            Attribute {
                name: "date",
                annotation: "str",
            },
            Attribute {
                name: "series",
                annotation: "str",
            },
            Attribute {
                name: "bar",
                annotation: "Bar",
            },
            Attribute {
                name: "prev_close",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "vwap",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "trades",
                annotation: "int | None",
            },
            Attribute {
                name: "deliverable_qty",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "deliverable_pct",
                annotation: "decimal.Decimal | None",
            },
        ],
        methods: &[],
    },
    Class {
        name: "SizingMethod",
        doc: "One dynamic position sizing model.",
        attributes: &[Attribute {
            name: "kind",
            annotation: "str",
        }],
        methods: &[
            Method {
                name: "fixed_fractional",
                signature: "def fixed_fractional(risk_pct: decimal.Decimal | int | str) -> SizingMethod: ...",
            },
            Method {
                name: "volatility_target",
                signature: "def volatility_target(target_annual_vol: decimal.Decimal | int | str, lookback_bars: int = 20) -> SizingMethod: ...",
            },
            Method {
                name: "kelly_criterion",
                signature: "def kelly_criterion(win_rate: decimal.Decimal | int | str, payoff_ratio: decimal.Decimal | int | str, fraction: decimal.Decimal | int | str | None = None) -> SizingMethod: ...",
            },
            Method {
                name: "equal_risk_contribution",
                signature: "def equal_risk_contribution() -> SizingMethod: ...",
            },
            Method {
                name: "fixed_notional",
                signature: "def fixed_notional(notional: decimal.Decimal | int | str | Money) -> SizingMethod: ...",
            },
            Method {
                name: "from_json",
                signature: "def from_json(text: str) -> SizingMethod: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "RiskBudget",
        doc: "Account equity, risk fraction and position limits for a sizing run.",
        attributes: &[
            Attribute {
                name: "equity",
                annotation: "Money",
            },
            Attribute {
                name: "risk_pct",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "min_lot_size",
                annotation: "Quantity",
            },
            Attribute {
                name: "max_position_notional",
                annotation: "Money | None",
            },
            Attribute {
                name: "max_position_qty",
                annotation: "Quantity | None",
            },
            Attribute {
                name: "max_portfolio_risk_pct",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "freeze_limit_qty",
                annotation: "Quantity | None",
            },
        ],
        methods: &[
            Method {
                name: "with_max_notional",
                signature: "def with_max_notional(self, max_notional: decimal.Decimal | int | str | Money) -> RiskBudget: ...",
            },
            Method {
                name: "with_max_qty",
                signature: "def with_max_qty(self, max_qty: decimal.Decimal | int | str) -> RiskBudget: ...",
            },
            Method {
                name: "with_freeze_limit",
                signature: "def with_freeze_limit(self, freeze_qty: decimal.Decimal | int | str) -> RiskBudget: ...",
            },
            Method {
                name: "with_max_portfolio_risk",
                signature: "def with_max_portfolio_risk(self, max_risk: decimal.Decimal | int | str) -> RiskBudget: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "PositionSizingRequest",
        doc: "A sizing question: instrument, side, entry, stop, equity, budget, method.",
        attributes: &[
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "side",
                annotation: "str",
            },
            Attribute {
                name: "entry_price",
                annotation: "Price",
            },
            Attribute {
                name: "stop_loss",
                annotation: "Price | None",
            },
            Attribute {
                name: "current_volatility",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "account_equity",
                annotation: "Money",
            },
            Attribute {
                name: "risk_budget",
                annotation: "RiskBudget",
            },
            Attribute {
                name: "sizing_method",
                annotation: "SizingMethod",
            },
        ],
        methods: &[
            Method {
                name: "with_stop_loss",
                signature: "def with_stop_loss(self, stop_loss: decimal.Decimal | int | str | Price) -> PositionSizingRequest: ...",
            },
            Method {
                name: "with_volatility",
                signature: "def with_volatility(self, volatility: decimal.Decimal | int | str) -> PositionSizingRequest: ...",
            },
            Method {
                name: "calculate",
                signature: "def calculate(self) -> PositionSizingResult: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "PositionSizingResult",
        doc: "The answer: a lot-rounded quantity plus its notional and risk.",
        attributes: &[
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "quantity",
                annotation: "Quantity",
            },
            Attribute {
                name: "allocated_notional",
                annotation: "Money",
            },
            Attribute {
                name: "risk_at_stake",
                annotation: "Money",
            },
            Attribute {
                name: "target_weight",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "capped_by_limit",
                annotation: "bool",
            },
            Attribute {
                name: "reason",
                annotation: "str | None",
            },
        ],
        methods: &[Method {
            name: "to_json",
            signature: "def to_json(self) -> str: ...",
        }],
    },
    Class {
        name: "Signal",
        doc: "A strategy signal: what to trade, in which direction, at which levels.",
        attributes: &[
            Attribute {
                name: "instrument",
                annotation: "InstrumentId",
            },
            Attribute {
                name: "direction",
                annotation: "str",
            },
            Attribute {
                name: "horizon",
                annotation: "str",
            },
            Attribute {
                name: "timeframe",
                annotation: "str",
            },
            Attribute {
                name: "strength",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "generated_by",
                annotation: "str",
            },
            Attribute {
                name: "timestamp",
                annotation: "str",
            },
            Attribute {
                name: "entry_price",
                annotation: "Price | None",
            },
            Attribute {
                name: "stop_loss",
                annotation: "Price | None",
            },
            Attribute {
                name: "take_profit",
                annotation: "Price | None",
            },
            Attribute {
                name: "volatility_atr",
                annotation: "decimal.Decimal | None",
            },
            Attribute {
                name: "side",
                annotation: "str | None",
            },
        ],
        methods: &[
            Method {
                name: "with_entry_price",
                signature: "def with_entry_price(self, price: decimal.Decimal | int | str | Price) -> Signal: ...",
            },
            Method {
                name: "with_stop_loss",
                signature: "def with_stop_loss(self, price: decimal.Decimal | int | str | Price) -> Signal: ...",
            },
            Method {
                name: "with_take_profit",
                signature: "def with_take_profit(self, price: decimal.Decimal | int | str | Price) -> Signal: ...",
            },
            Method {
                name: "with_volatility_atr",
                signature: "def with_volatility_atr(self, value: decimal.Decimal | int | str) -> Signal: ...",
            },
            Method {
                name: "to_json",
                signature: "def to_json(self) -> str: ...",
            },
        ],
    },
    Class {
        name: "FixedFractionalSizer",
        doc: "Risk a fixed fraction of equity against the stop distance.",
        attributes: &[
            Attribute {
                name: "risk_pct",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "lot_size",
                annotation: "Quantity",
            },
        ],
        methods: &[Method {
            name: "calculate_size",
            signature: "def calculate_size(self, instrument: InstrumentId | str, entry_price: decimal.Decimal | int | str | Price, equity: decimal.Decimal | int | str | Money, side: str = 'BUY', stop_loss: decimal.Decimal | int | str | Price | None = None) -> PositionSizingResult: ...",
        }],
    },
    Class {
        name: "AtrVolatilitySizer",
        doc: "Size inversely proportional to the ATR band.",
        attributes: &[
            Attribute {
                name: "target_risk",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "multiplier",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "lot_size",
                annotation: "Quantity",
            },
        ],
        methods: &[Method {
            name: "calculate_size",
            signature: "def calculate_size(self, instrument: InstrumentId | str, entry_price: decimal.Decimal | int | str | Price, equity: decimal.Decimal | int | str | Money, atr: decimal.Decimal | int | str, side: str = 'BUY', stop_loss: decimal.Decimal | int | str | Price | None = None) -> PositionSizingResult: ...",
        }],
    },
    Class {
        name: "KellySizer",
        doc: "Damped Kelly growth sizing.",
        attributes: &[
            Attribute {
                name: "win_rate",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "payoff_ratio",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "fraction",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "lot_size",
                annotation: "Quantity",
            },
        ],
        methods: &[
            Method {
                name: "kelly_fraction",
                signature: "def kelly_fraction(self) -> decimal.Decimal: ...",
            },
            Method {
                name: "calculate_size",
                signature: "def calculate_size(self, instrument: InstrumentId | str, entry_price: decimal.Decimal | int | str | Price, equity: decimal.Decimal | int | str | Money, side: str = 'BUY', stop_loss: decimal.Decimal | int | str | Price | None = None) -> PositionSizingResult: ...",
            },
        ],
    },
    Class {
        name: "EqualRiskSizer",
        doc: "Inverse-volatility risk parity across active signals.",
        attributes: &[
            Attribute {
                name: "risk_pct",
                annotation: "decimal.Decimal",
            },
            Attribute {
                name: "lot_size",
                annotation: "Quantity",
            },
        ],
        methods: &[Method {
            name: "allocate",
            signature: "def allocate(self, equity: decimal.Decimal | int | str | Money, instruments: list[InstrumentId | str], entry_prices: list[decimal.Decimal | int | str | Price], volatilities: list[decimal.Decimal | int | str], gross_exposure: decimal.Decimal | int | str = 1) -> list[PositionSizingResult]: ...",
        }],
    },
    Class {
        name: "FixedNotionalSizer",
        doc: "Allocate a fixed cash notional per trade, lot-rounded.",
        attributes: &[
            Attribute {
                name: "notional",
                annotation: "Money",
            },
            Attribute {
                name: "lot_size",
                annotation: "Quantity",
            },
        ],
        methods: &[Method {
            name: "calculate_size",
            signature: "def calculate_size(self, instrument: InstrumentId | str, entry_price: decimal.Decimal | int | str | Price, side: str = 'BUY') -> PositionSizingResult: ...",
        }],
    },
];

/// The module-level functions.
pub const FUNCTIONS: &[Function] = &[
    Function {
        name: "canonical_json",
        signature: "def canonical_json(kind: str, payload: str) -> str: ...",
    },
    Function {
        name: "wire_enum_values",
        signature: "def wire_enum_values() -> dict[str, list[str]]: ...",
    },
    Function {
        name: "currency_minor_units",
        signature: "def currency_minor_units() -> dict[str, tuple[int, str, str]]: ...",
    },
    Function {
        name: "verify_manifest",
        signature: "def verify_manifest(manifest_json: str) -> str: ...",
    },
    Function {
        name: "codegen_render",
        signature: "def codegen_render(kind: str) -> tuple[str, str]: ...",
    },
    Function {
        name: "codegen_artifacts",
        signature: "def codegen_artifacts() -> list[str]: ...",
    },
    Function {
        name: "parse_fno_symbol",
        signature: "def parse_fno_symbol(symbol: str) -> FnoContract: ...",
    },
    Function {
        name: "nse_equity_settlement_days",
        signature: "def nse_equity_settlement_days(as_of: str | None = None) -> int: ...",
    },
    Function {
        name: "runtime_start",
        signature: "def runtime_start(worker_threads: int | None = None) -> tuple[str, int, int]: ...",
    },
    Function {
        name: "runtime_stop",
        signature: "def runtime_stop() -> bool: ...",
    },
    Function {
        name: "runtime_info",
        signature: "def runtime_info() -> tuple[str, int, int] | None: ...",
    },
    Function {
        name: "protocol_capsule",
        signature: "def protocol_capsule() -> ArrowStream: ...",
    },
    Function {
        name: "bars_to_arrow",
        signature: "def bars_to_arrow(bars: list[Bar]) -> ArrowStream: ...",
    },
    Function {
        name: "ticks_to_arrow",
        signature: "def ticks_to_arrow(ticks: list[Tick]) -> ArrowStream: ...",
    },
    Function {
        name: "export_capsule",
        signature: "def export_capsule(bars: list[Bar]) -> Any: ...",
    },
    Function {
        name: "parse_bhavcopy_csv",
        signature: "def parse_bhavcopy_csv(csv_text: str, fallback_date: str | None = None, delivery_dat: str | None = None) -> list[BhavRecord]: ...",
    },
    Function {
        name: "bhavcopy_to_bars",
        signature: "def bhavcopy_to_bars(csv_text: str, fallback_date: str | None = None, series: str | None = 'EQ') -> list[Bar]: ...",
    },
    Function {
        name: "bhavcopy_to_arrow",
        signature: "def bhavcopy_to_arrow(csv_text: str, fallback_date: str | None = None, series: str | None = 'EQ') -> ArrowStream: ...",
    },
    Function {
        name: "batch_sma",
        signature: "def batch_sma(values: list[float], period: int) -> list[float | None]: ...",
    },
    Function {
        name: "batch_ema",
        signature: "def batch_ema(values: list[float], period: int) -> list[float | None]: ...",
    },
    Function {
        name: "batch_rma",
        signature: "def batch_rma(values: list[float], period: int) -> list[float | None]: ...",
    },
    Function {
        name: "batch_rsi",
        signature: "def batch_rsi(values: list[float], period: int) -> list[float | None]: ...",
    },
    Function {
        name: "batch_bollinger",
        signature: "def batch_bollinger(values: list[float], period: int = 20, num_std: float = 2.0) -> tuple[list[float | None], list[float | None], list[float | None]]: ...",
    },
    Function {
        name: "batch_macd",
        signature: "def batch_macd(values: list[float], fast_period: int = 12, slow_period: int = 26, signal_period: int = 9) -> tuple[list[float | None], list[float | None], list[float | None]]: ...",
    },
    Function {
        name: "batch_atr",
        signature: "def batch_atr(high: list[float], low: list[float], close: list[float], period: int = 14) -> list[float | None]: ...",
    },
    Function {
        name: "batch_donchian",
        signature: "def batch_donchian(high: list[float], low: list[float], period: int = 20) -> tuple[list[float | None], list[float | None], list[float | None]]: ...",
    },
    Function {
        name: "batch_stochastic",
        signature: "def batch_stochastic(high: list[float], low: list[float], close: list[float], k_period: int = 14, d_period: int = 3) -> tuple[list[float | None], list[float | None]]: ...",
    },
    Function {
        name: "batch_williams_r",
        signature: "def batch_williams_r(high: list[float], low: list[float], close: list[float], period: int = 14) -> list[float | None]: ...",
    },
    Function {
        name: "batch_supertrend",
        signature: "def batch_supertrend(high: list[float], low: list[float], close: list[float], period: int = 10, multiplier: float = 3.0) -> tuple[list[float | None], list[int]]: ...",
    },
    Function {
        name: "batch_vwap",
        signature: "def batch_vwap(high: list[float], low: list[float], close: list[float], volume: list[float]) -> list[float | None]: ...",
    },
    Function {
        name: "batch_cpr",
        signature: "def batch_cpr(high: list[float], low: list[float], close: list[float]) -> tuple[list[float | None], list[float | None], list[float | None], list[float | None]]: ...",
    },
    Function {
        name: "batch_adx",
        signature: "def batch_adx(high: list[float], low: list[float], close: list[float], period: int = 14) -> list[float | None]: ...",
    },
    Function {
        name: "batch_obv",
        signature: "def batch_obv(close: list[float], volume: list[float]) -> list[float | None]: ...",
    },
    Function {
        name: "size_batch",
        signature: "def size_batch(instruments: list[InstrumentId | str], entry_prices: list[decimal.Decimal | int | str | Price], equity: decimal.Decimal | int | str | Money, risk_pct: decimal.Decimal | int | str | None = None, lot_size: decimal.Decimal | int | str | None = None, stop_losses: list[decimal.Decimal | int | str | Price | None] | None = None, volatilities: list[decimal.Decimal | int | str | None] | None = None, method: str = 'fixed_fractional') -> list[PositionSizingResult]: ...",
    },
];

/// Renders the native module as a strict `.pyi` stub.
pub fn render_pyi() -> String {
    let mut out = String::new();
    out.push_str("# Generated by honba-codegen from the Rust native surface.\n");
    out.push_str("# DO NOT EDIT: regenerate with `make pyi` instead.\n\n");
    out.push_str("from __future__ import annotations\n\n");
    out.push_str("import datetime\n");
    out.push_str("import decimal\n");
    out.push_str("from typing import Any\n\n");

    for (name, annotation) in CONSTANTS {
        out.push_str(&format!("{name}: {annotation}\n"));
    }
    out.push('\n');

    for class in CLASSES {
        out.push_str(&format!("class {}:\n", class.name));
        out.push_str(&format!("    \"\"\"{}\"\"\"\n", class.doc));
        if class.attributes.is_empty() && class.methods.is_empty() {
            out.push_str("    pass\n\n");
            continue;
        }
        if !class.attributes.is_empty() {
            out.push('\n');
            for attribute in class.attributes {
                out.push_str(&format!(
                    "    {}: {}\n",
                    attribute.name, attribute.annotation
                ));
            }
        }
        if !class.methods.is_empty() {
            out.push('\n');
            for method in class.methods {
                out.push_str(&format!("    {}\n", method.signature));
            }
        }
        out.push('\n');
    }

    for function in FUNCTIONS {
        out.push_str(&format!("{}\n", function.signature));
    }

    out
}

/// Every name the native module exports, for the conformance test.
pub fn declared_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = CONSTANTS.iter().map(|(name, _)| *name).collect();
    names.extend(CLASSES.iter().map(|class| class.name));
    names.extend(FUNCTIONS.iter().map(|function| function.name));
    names
}
