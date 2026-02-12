# 002 — Format-on-Display

**Priority:** Critical
**Scope:** Display system, REPL rendering
**Depends on:** 001 (Record and Table Types)

---

## Summary

Structured data flows through the pipeline as rich typed values. Only at the terminal — when a value would be printed — does the display formatter render it. Intermediate pipeline stages never see rendered text. This is PowerShell's most important design insight.

## How it works

When a command completes and the result would be shown to the user:

1. Inspect the value type
2. Choose a formatter:
   - `List<Record>` with uniform keys → table with box-drawing characters
   - `Record` → aligned key-value pairs
   - `List<Str/Int/Float>` → newline-separated
   - `Str` → as-is
   - `Error` → red with message, source, code
   - `Nothing` → nothing displayed
3. Render to terminal
4. **Do not modify the stack value.** The rich data remains available.

## Table rendering

```
┌────────────┬──────┬───────┐
│ name       │ type │ size  │
├────────────┼──────┼───────┤
│ Cargo.toml │ file │ 1234  │
│ src/       │ dir  │ 4096  │
└────────────┴──────┴───────┘
```

Use the `comfy-table` or `tabled` crate. Handle terminal width gracefully — truncate columns or wrap rather than breaking layout.

## REPL `.s` display

`.s` should show type-annotated previews:

```
| Table(3×4) Record{name,ver} "hello"(str) 42(int) [echo](block) |
```

## User overrides

`to-table`, `to-list`, `to-json`, `to-csv`, `to-tsv` force specific formats. These produce Str values (the formatted text), unlike the display formatter which only affects terminal output.

## Key constraint

Format-on-display must never alter the underlying data. `ls` pushes a table. The table is displayed. The table is still on the stack with all fields intact for the next operation. This is the entire point.
