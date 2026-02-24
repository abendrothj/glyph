//! Built-in names to exclude from call graphs (std lib, methods, etc.).
//! Uses phf for compile-time static sets — no allocation, O(1) lookup.

use phf::phf_set;

/// Python built-ins and common stdlib.
pub static PYTHON_BUILTINS: phf::Set<&'static str> = phf_set! {
    "len", "print", "str", "int", "float", "bool", "list", "dict", "set", "tuple",
    "range", "map", "filter", "zip", "enumerate", "reversed", "sorted", "sum", "min", "max",
    "abs", "round", "open", "input", "type", "isinstance", "hasattr", "getattr", "setattr",
    "repr", "format", "iter", "next", "all", "any", "callable", "chr", "ord", "divmod",
    "hash", "id", "issubclass", "locals", "globals", "vars", "dir",
    "super", "property", "staticmethod", "classmethod", "object", "Exception", "BaseException",
    "slice", "frozenset", "bytes", "bytearray", "memoryview", "complex", "pow",
    "exit", "quit", "help", "license", "copyright", "credits",
};

/// Rust std + Bevy ECS methods (as_mut, unwrap, spawn, single, etc.).
pub static RUST_BUILTINS: phf::Set<&'static str> = phf_set! {
    "as_mut", "as_ref", "unwrap", "expect", "unwrap_or", "unwrap_or_else",
    "ok", "err", "clone", "copy", "len", "is_empty", "iter", "into_iter",
    "get", "get_mut", "insert", "remove", "contains", "push", "pop",
    "to_string", "to_owned", "borrow", "borrow_mut", "deref", "deref_mut",
    "default", "new", "from", "into", "try_into", "try_from",
    "eq", "ne", "lt", "le", "gt", "ge", "cmp", "partial_cmp",
    "hash", "fmt", "debug", "display", "send", "sync",
    // Bevy ECS
    "spawn", "despawn", "entity", "single", "single_mut", "iter_mut",
    "add_systems", "add_plugins", "add_system", "run_if", "in_state",
    "insert_resource", "init_resource", "add_message", "write_message",
    "add_child", "remove_children", "with_children",
};

/// TypeScript/JavaScript built-ins and common methods.
pub static TYPESCRIPT_BUILTINS: phf::Set<&'static str> = phf_set! {
    "log", "warn", "error", "info", "debug", "trace",
    "parseInt", "parseFloat", "isNaN", "isFinite", "eval",
    "map", "filter", "reduce", "find", "findIndex", "forEach",
    "push", "pop", "shift", "unshift", "splice", "slice",
    "concat", "join", "reverse", "sort", "includes", "indexOf",
    "length", "toString", "valueOf", "hasOwnProperty", "isPrototypeOf",
    "keys", "values", "entries", "assign", "freeze", "seal",
    "parse", "stringify", "then", "catch", "finally", "resolve", "reject",
    "Array", "Object", "String", "Number", "Boolean", "Math", "JSON", "Promise",
    "console", "setTimeout", "setInterval", "clearTimeout", "clearInterval",
};
