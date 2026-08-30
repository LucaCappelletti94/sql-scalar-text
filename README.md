# sql-scalar-text

`sql-scalar-text` parses the text forms that PostgreSQL, MySQL, and SQLite emit for scalar values. Its shared acceptance set lets every wire-text consumer decode the same spelling to the same Rust value.

```rust
assert_eq!(sql_scalar_text::parse_bool("t"), Some(true));
```

The crate supports `no_std`. Decimal parsing is available through the `decimal` feature.
