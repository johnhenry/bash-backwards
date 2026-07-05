# 003 — External Command Interop

**Priority:** Critical
**Scope:** Pipeline boundary between structured data and external processes
**Depends on:** 001 (Record and Table Types)

---


## Status (as of reconciliation — wave 2, 2026-07)

**Largely implemented**, with one deliberate deviation:

- **Outbound auto-serialization** — `Value::as_arg()` (`src/ast.rs`):
  tables serialize to TSV with a header row, lists join with newlines,
  records to `key=value` lines (nested records to JSON), numbers/strings
  as expected. Exactly the table below.
- **Inbound explicit parse** — external output enters as `Output`
  (text); `from-json`/`from-csv`/`from-tsv`/`json` upgrade it explicitly
  (`src/eval/serialization.rs`). No auto-parsing heuristics.
- **Byte fidelity + structured failure (#25)** — non-UTF-8 stdout is
  preserved as `Value::Bytes`; a non-zero exit pushes
  `Value::Error{kind:"command", message:<stderr>, code, command}`
  (`src/eval/command.rs`).
- **Deviation:** the `raw` escape hatch was not built. Instead of making
  `ls`/`ps` structured-by-default, structured variants are additive
  (`ls-t`, `ps-t`, `env-t`, `which-t`, `history-t` — #27), so text-mode
  scripts never need an escape hatch.

---

## Summary

Define how structured data crosses the boundary to external commands and how external command output enters the structured world. This is the make-or-break decision for usability.

## Core principle: Asymmetric serialization

- **Structured → external:** auto-serialize. Always works, user doesn't think about it.
- **External → structured:** explicit parse. User decides when to upgrade text to typed data.

## Outbound auto-serialization

| Type | Default format |
|------|---------------|
| `List<Record>` (table) | TSV with header row |
| `List<Str>` | Newline-separated |
| `List<Int/Float>` | Newline-separated |
| `Record` | `key=value` lines |
| `Str` | As-is |
| `Int/Float` | String representation |
| `Nothing` | Empty string |

TSV is the default for tables because `awk`, `cut`, `sort`, and `join` all work with tab-separated data natively. CSV has quoting issues. JSON requires `jq`.

Users can override with explicit serializers: `to-csv`, `to-json`, `to-ndjson`.

## Inbound: always text unless parsed

External command output is always `Str` on the stack. Parse explicitly:

```
curl -s url json                  # explicit JSON parse
cat data.csv into csv             # explicit CSV parse
ps aux into tsv                   # explicit TSV parse
```

Never auto-parse. Heuristics will be wrong. Explicit parsing makes scripts predictable.

## Why this matters

Nushell sends its formatted table to external commands (lossy, unparseable). PowerShell sends `.ToString()` (useless type names). Both are wrong. TSV auto-serialization gives external tools clean, parseable tabular data by default.

## The `raw` escape hatch

```
raw ls          # text output, not structured
raw [block]     # entire block in text mode
```
