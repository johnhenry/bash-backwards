# 003 — External Command Interop

**Priority:** Critical
**Scope:** Pipeline boundary between structured data and external processes
**Depends on:** 001 (Record and Table Types)

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
