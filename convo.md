# hsab — Conversation: Analysis, Improvements, and Structured Data Design

## How well does hsab achieve its stated goals?

The README's design philosophy lists five principles. Here's how honestly it delivers on each:

**1. Stack semantics** — Strong. The stack model is well-defined and consistent. Stack ops (`dup`, `swap`, `rot`, etc.), markers, `spread`/`each`/`keep`/`collect`, and arithmetic all work naturally on the stack. The Forth heritage is clear and coherent.

**2. Output threading** — Mostly strong, but the cognitive overhead is real. `pwd ls` meaning `ls $(pwd)` is elegant. But LIFO ordering means you have to think backwards about argument order for multi-arg commands, and the README itself keeps needing to add comments like `# world pushed first` to clarify. The `swap` escape hatch (`dest src swap mv`) is honest about the ergonomic cost.

**3. Deferred execution** — Strong. Blocks as first-class values is the best part of the design. Using `[...]` for pipes, conditionals, and definitions is genuinely more composable than bash's approach. The `@` apply operator is clean.

**4. Explicit control** — Mixed. Operators do make flow visible, but the pipe syntax `ls [grep Cargo] |` is arguably *less* readable than `ls | grep Cargo`. The postfix pipe trades unix familiarity for consistency with the stack model — a defensible choice, but "explicit" is doing heavy lifting when readers have to mentally reverse everything.

**5. Standalone / No bash dependency** — Undermined by the `bash` builtin. The README includes a whole section on using `bash` as a fallback for brace expansion, process substitution, for-loops, and here-strings. This is pragmatic, but it's an admission that the postfix model can't cover real shell workflows without an escape hatch back to the thing it's replacing.

**Overall:** The core model is intellectually coherent and well-implemented. The weak point is the *pitch* — the README is more of a reference manual than a persuasive document. It doesn't answer "why would I switch?" with compelling before/after comparisons. And the "Common Gotchas" section (where `yes`, `time`, `file`, `sort`, `join`, `split` are all dangerous unquoted) is a significant usability red flag that the README buries near the bottom rather than addressing head-on.

---

## Better Examples

The current "Examples" section just re-demonstrates basic syntax. Here are examples that actually show workflows where hsab's model pays off:

### Batch rename files (stack + each)
```
# Rename all .txt files to .md
-1 ls spread ["." rsplit1 drop ".md" suffix swap mv] each

# Bash equivalent:
# for f in *; do mv "$f" "${f%.txt}.md"; done
```

### Parallel health checks
```
[["api.example.com" ping] ["db.example.com" ping] ["cache.example.com" ping]] parallel
```

### Safe backup before editing
```
[dup dup .bak suffix cp vim] :edit-safe
# Usage: config.yaml edit-safe
```

### Filter and process pipeline
```
"." find spread ["-f" test] keep [-h du] each
```

### Diff two directories without bash process substitution
```
[/etc/nginx/sites-available ls] subst [/etc/nginx/sites-enabled ls] subst diff
```

### Conditional deploy with git status
```
[.git/refs/heads/main -f test]
  ["main" git-checkout "origin" "main" git-push "Deployed." echo]
  ["Not a git repo." echo]
if
```

### Composable definitions that chain
```
[-1 ls spread [-f test] keep collect] :files
[-1 ls spread [-d test] keep collect] :dirs
files [wc -l] |
dirs [head -5] |
```

### Arithmetic in a real context
```
-1 ls spread depth
dup 10 gt?
  [" files — that's a lot." suffix echo]
  [" files." suffix echo]
if
```

---

## Bash Equivalents for Every Example

### Batch rename files
```
# hsab
-1 ls spread ["." rsplit1 drop ".md" suffix swap mv] each

# bash
for f in *; do mv "$f" "${f%.txt}.md"; done
```

### Parallel health checks
```
# hsab
[["api.example.com" ping] ["db.example.com" ping] ["cache.example.com" ping]] parallel

# bash
ping api.example.com & ping db.example.com & ping cache.example.com & wait
```

### Safe backup before editing
```
# hsab
[dup dup .bak suffix cp vim] :edit-safe
config.yaml edit-safe

# bash
edit-safe() { cp "$1" "$1.bak" && vim "$1"; }
edit-safe config.yaml
```

### Filter and process pipeline
```
# hsab
"." find spread ["-f" test] keep [-h du] each

# bash
find . -type f -exec du -h {} +
```

### Diff two directories
```
# hsab
[/etc/nginx/sites-available ls] subst [/etc/nginx/sites-enabled ls] subst diff

# bash
diff <(ls /etc/nginx/sites-available) <(ls /etc/nginx/sites-enabled)
```

### Conditional deploy
```
# hsab
[.git/refs/heads/main -f test]
  ["main" git-checkout "origin" "main" git-push "Deployed." echo]
  ["Not a git repo." echo]
if

# bash
if [ -f .git/refs/heads/main ]; then
  git checkout main && git push origin main && echo "Deployed."
else
  echo "Not a git repo."
fi
```

### Composable definitions that chain
```
# hsab
[-1 ls spread [-f test] keep collect] :files
[-1 ls spread [-d test] keep collect] :dirs
files [wc -l] |
dirs [head -5] |

# bash
files() { find . -maxdepth 1 -type f; }
dirs() { find . -maxdepth 1 -type d | tail -n +2; }
files | wc -l
dirs | head -5
```

### Arithmetic in a real context
```
# hsab
-1 ls spread depth
dup 10 gt? [" files — that's a lot." suffix echo] [" files." suffix echo] if

# bash
count=$(ls -1 | wc -l)
if [ "$count" -gt 10 ]; then
  echo "$count files — that's a lot."
else
  echo "$count files."
fi
```

**Assessment:** The bash versions are shorter or comparably readable in most cases. hsab wins on composable definitions and parallel execution. The strongest case for hsab isn't any single example — it's the persistent interactive stack, which bash truly cannot do.

---

## Where hsab Really Shines

### 1. Reusing a value multiple times without temp variables

```
# hsab — one command, result used three times
find . -name "*.log" -newer /tmp/marker wc -l dup dup
"Found " swap " log files" suffix suffix echo
10 gt? ["Rotating..." echo] [] if
[rotate-logs] [] if

# bash — must capture, then reference repeatedly
count=$(find . -name "*.log" -newer /tmp/marker | wc -l)
echo "Found $count log files"
if [ "$count" -gt 10 ]; then echo "Rotating..."; rotate-logs; fi
```

### 2. Interactive exploration with persistent stack

This is hsab's killer feature with no bash equivalent.

```
hsab¢ -1 ls spread              # all files now on stack
hsab¢ [-f test] keep            # filter to regular files
hsab¢ .s                        # inspect what's left
hsab¢ [".rs" ends?] keep        # narrow further
hsab¢ .s                        # check again
hsab¢ [-l wc] each             # count lines in each
```

In bash, every refinement means retyping or up-arrowing and editing the full pipeline.

### 3. Operating on pairs/groups of related values

```
-1 ls spread [
  dup "." split1 drop "_processed.csv" suffix swap mv
] each
```

No named variables — `dup` and `swap` manage "I need the original AND a derived version" purely through stack position.

### 4. Higher-order operations (passing behavior as data)

```
[
  swap 0
  [
    over @ dup 0 =? [drop drop drop return] [] if
    drop 1 plus
    2dup le? [] [drop drop "Failed" echo 1 return] if
    1 sleep
  ] while
] :retry

3 [curl -s https://api.example.com/health] retry
```

The command block is a value you can `dup`, pass to multiple retriers, compose with other blocks.

### 5. Fan-out to multiple destinations

```
# hsab
[date] [logs/a.log logs/b.log logs/c.log] >

# bash
date | tee logs/a.log logs/b.log > logs/c.log
```

### 6. Process substitution without bash dependency

```
[sort < users_old.txt] subst [sort < users_new.txt] subst comm
```

---

## Other Advantages Over Bash

**Uniform syntax.** Bash has ~six different sublanguages. hsab has one model: push values, pop values, apply blocks.

**Blocks are values, not strings.** No eval, no `"$@"`, no quoting hell.

**No quoting catastrophes.** Values on the stack are discrete items, not whitespace-delimited strings waiting to explode.

**Parallel execution is a primitive.** `parallel` runs blocks concurrently and pushes all outputs to the stack.

**The stack is a clipboard.** Infinite clipboard between interactive commands.

**Definitions are cleaner than functions.** `[-la ls] :ll` vs `ll() { ls -la "$@"; }`

**What's NOT an advantage:** "No bash dependency" isn't practically valuable. Postfix notation is a tradeoff, not an advantage.

---

## Minor Updates to Improve the Application

### README/positioning (no code)
- Lead with the interactive REPL story — that's the hook
- Rename the project ("Bash Backwards" frames it as a novelty)
- Add "When to use hsab / When to stick with bash" section

### Small features
- `peek` — show top N stack items without popping, usable in scripts
- `--trace` mode — show each step's stack state for debugging/learning
- Stack type annotations in REPL hint: `| "myfile.txt"(str) 42(num) [echo](block) |`

### Ergonomic fixes
- Argument-order modifier (`<mv`) to reduce `swap` gymnastics
- Multi-file redirect should require explicit list marker
- Default unknown words to literals (sigil for forced execution) to avoid quoting gotchas

### stdlib improvements
- Ship stdlib by default (no `hsab init` required)
- Add `tap` — execute block for side effect, keep original value
- Add `dip` — pop top, execute block, push value back (from Factor)

### Error handling
- Define behavior when commands fail mid-`each`
- Add `try`/`catch` or `on-error` block semantics

---

## Larger Improvements

### Structured data (object model)
- `Record` type (key-value map) and `Table` type (list of records)
- Operations: `get`, `set`, `keys`, `values`, `where`, `sort-by`
- Structured data auto-serializes to text when hitting external commands
- External output stays text unless explicitly parsed

### Ecosystem
- Plugin/extension system (Rust shared libraries)
- Tab completion
- Namespaces for definitions
- String interpolation
- Step debugger
- Stack overflow protection
- WASM playground

### Adoption
- Real-world example scripts
- Comparison page vs bash, fish, zsh, Nushell, xonsh
- Migration guide for 20 most common bash patterns

### Strategic positioning
The structured data angle is the most defensible direction — a stack-based shell with native typed data, JSON operations, and parallel primitives could be genuinely unique. Trying to be "bash but postfix" is a losing proposition.

---

## Structured Data Design — Examples

```
# --- Basic table operations ---

ls
# ┌────────────┬──────┬───────┬─────────────────────┐
# │ name       │ type │ size  │ modified            │
# └────────────┴──────┴───────┴─────────────────────┘

ls "name" get                # get a single column
ls ["size" get 2000 gt?] keep  # filter rows
ls "size" sort-by             # sort by column

# --- Deep access ---
'{"server": {"host": "localhost", "port": 8080}}' json
"server.port" get            # 8080

# --- Records ---
{"name" "hsab" "version" "0.2.0"} record
"name" get                   # hsab

# --- Stack advantage: multiple tables ---
ls ~/projects/alpha
ls ~/projects/beta
"name" get swap "name" get
diff                         # files in alpha but not beta

# --- API workflows ---
"https://api.github.com/users/johnhenry/repos" fetch json
["language" get "Rust" eq?] keep
"name" get

# --- Multiple APIs on stack ---
"https://api.example.com/users" fetch json
"https://api.example.com/orders" fetch json
"user_id" "id" join-on
["status" get "active" eq?] keep
"total" get sum

# --- CSV/JSON interop ---
"sales.csv" open
"region" group-by ["amount" get sum] each
unjson "summary.json" write

# --- Graceful degradation to text ---
ls "name" get [grep "test"] |    # auto-serializes for external commands
ls to-csv                        # explicit format
ls to-json
```

---

## External Command Interop Design

### Core principle
- **structured → external:** auto-serialize to text
- **external → structured:** stays text unless explicitly parsed
- Never auto-parse. Always auto-serialize.

### The `into` bridge operator
```
ps aux [into tsv] |             # TSV → table
env [into kv "="] |             # key=value → record
cat sales.csv [into csv] |     # CSV → table
df -h [into columns] |         # fixed-width → table
curl -s url [into json] |      # JSON → structured
```

### Mixed workflows
```
# git log → structured → filtered → text
git log --format="%H|%an|%s|%ad" [into csv "|"] |
["author" get "john" eq?] keep
["subject" get] each
[head -5] |

# docker containers as a table
docker ps --format '{{json .}}' spread [json] each collect
["Status" get "Up" starts?] keep
"Names" get
```

### Escape hatch
```
raw ls                         # force text mode
raw [ls [grep foo] |]          # entire block in text mode
```

---

## Ideas from PowerShell

### Format-on-display (highest priority)
Objects flow as rich objects through the pipeline. Only at the terminal does formatting kick in. Intermediate stages never lose information.

### Error records (second priority)
```
ls /nonexistent
# stack: [ErrorRecord(message: "not found", path: "/nonexistent", code: 2)]
error?                       # true
"message" get                # "not found"
```

### Calculated properties (third priority)
```
ls ["size" get 1024 div] "size_kb" add-column
ls "size" [1024 div] map-column
```

### Objects with methods (ambitious)
```
[
  {"name" "id" "status"} fields
  [dup "id" get docker stop] :stop method
  [dup "id" get docker logs] :logs method
] :Container

docker-ps [.stop] each
```

### Providers (ambitious)
```
fs:/var/log ls               # filesystem
env: ls                      # environment variables as records
git: ls                      # git objects
docker: ls                   # containers/images/volumes
```

### Key lessons from PowerShell
- Structured pipeline was technically superior but lost on Linux due to ecosystem lock-in and verbosity
- hsab's postfix syntax is terser, which helps
- **Critical:** keep external tool interop frictionless

---

## Nushell Overview

Nushell is a structured-data shell (Rust, ~2019) where commands return tables with typed columns instead of text.

**Strengths:** Typed pipelines, native JSON/YAML/CSV/TOML/SQLite, excellent errors, module system.

**Weaknesses:** Zero POSIX compatibility, interactive experience behind fish, memory-bound for large data, small ecosystem.

**vs hsab:** Nushell's insight: "data should be structured." hsab's insight: "flow should be stack-based." The interesting direction is combining both — a stack-based shell with typed/structured values enabling non-linear data flow that Nushell's linear pipeline can't express cleanly.
