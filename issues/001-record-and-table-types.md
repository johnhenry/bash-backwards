# 001 — Record and Table Types

**Priority:** Critical
**Scope:** Runtime type system
**Depends on:** 000 (Structured Object Model)
**Blocks:** 002, 003, 004, 005, 006

---

## Summary

Implement `Record` (ordered key-value map) and `List` (ordered sequence) as first-class `Value` variants. A table is a `List<Record>` where all records share the same keys — no separate Table type needed.

## Implementation

Add to the `Value` enum:

```rust
Record(IndexMap<String, Value>),
List(Vec<Value>),
```

Use the `indexmap` crate for insertion-order preservation with O(1) lookup.

## New keywords

- `record` — consumes pairs from the stack (or from a brace-delimited literal) and builds a Record
- `list` — consumes items from a brace-delimited literal and builds a List
- `get` — polymorphic: Record → extract field, List<Record> → extract column
- `set` — returns new Record with updated field (immutable)
- `keys` — pushes List<Str> of record keys
- `values` — pushes List of record values
- `count` — pushes Int length of List or Record

## Polymorphic `get`

`get` is the most important operation. On a Record, it extracts a single field. On a List<Record> (table), it maps `get` over each record and returns a List. This means `ls "name" get` works whether `ls` returns one record or a table.

Dot-path syntax (`"server.port" get`) navigates nested records by splitting on `.` and chaining gets.

`get?` (optional get) pushes `Nothing` on missing keys instead of erroring.

## Interaction with existing stack ops

`dup`, `swap`, `rot`, `drop`, `over` all work unchanged — they operate on Values regardless of type. `spread` on a List pushes each element individually. `collect` gathers N items into a List.

## Tests

- Construct a record, get a field, get a missing field (error)
- Construct a list, spread it, collect it back
- Construct a table (list of records), get a column
- Nested record with dot-path get
- `set` returns new record, original unchanged on stack via `dup`
