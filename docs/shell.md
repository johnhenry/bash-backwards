# Shell Migration Guide: bash/zsh to hsab

This guide helps bash and zsh users transition to hsab. Most shell knowledge transfers directly; the main difference is postfix notation and the persistent stack.

## Running Commands

### Commands Work Like Normal

Simple commands execute exactly as in bash:

```bash
ls
cat file.txt
git status
curl https://example.com
```

### Arguments in Postfix

For hsab-native style, arguments come before the command:

```bash
# bash style (still works)
ls -la
cat file.txt

# hsab postfix style
-la ls
file.txt cat
```

### Mixing Styles

You can mix freely. Use whatever feels natural:

```bash
# All equivalent
git log --oneline -10
--oneline -10 log git

# Partial postfix
-la ls | grep .rs
```

**When to use postfix:** When you want results on the stack for further manipulation.

```bash
# Postfix puts result on stack
file.txt cat               # content on stack
upper                      # transform it
.bak suffix                # add extension
```

---

## Pipes and Redirection

### Pipes

Two syntaxes available:

```bash
# Traditional (infix)
cat file.txt | grep error | sort

# Postfix with blocks
[cat file.txt] [grep error] |
[sort] |
```

The postfix form is useful when building pipelines dynamically:

```bash
# Store a pipeline stage
[grep -v DEBUG] :filter-logs

# Use it
cat app.log | filter-logs @
```

### Output Redirection

```bash
# Write to file (overwrite)
[ls -la] "listing.txt" >
ls -la > listing.txt           # traditional also works

# Append to file
[date] "log.txt" >>
date >> log.txt

# Write stack value to file
"Hello, world!" "greeting.txt" >
```

### Input Redirection

```bash
# Read file as input to command
"data.txt" [sort] <
sort < data.txt                # traditional also works
```

### Stderr Handling

```bash
# Redirect stderr to file
[make] "errors.txt" 2>

# Redirect stderr to stdout
[make] 2>&1

# Redirect all output
[make] "output.txt" &>

# Discard stderr
[noisy-command] /dev/null 2>
```

---

## Environment Variables

### Reading Variables

```bash
# Standard expansion
$HOME
$PATH
${USER}

# Use in commands
$HOME cd
$HOME/.config ls
```

### Setting Variables

```bash
# Set a variable (persists in session)
"value" "VAR" setenv

# Or use .export
.export VAR=value

# Examples
"/opt/bin:$PATH" "PATH" setenv
"production" "ENV" setenv
```

### Listing Variables

```bash
# Show all environment variables
env

# Filter for specific ones
env | grep PATH
```

### Unsetting Variables

```bash
.unset VAR
```

---

## Globs and Expansion

### Wildcards

Standard glob patterns work:

```bash
# Match any characters
*.txt ls                   # all .txt files
src/*.rs ls                # .rs files in src/

# Recursive glob
**/*.md ls                 # all .md files, any depth

# Single character
file?.txt ls               # file1.txt, fileA.txt, etc.

# Character classes
file[0-9].txt ls           # file0.txt through file9.txt
file[abc].txt ls           # filea.txt, fileb.txt, filec.txt
```

### Tilde Expansion

```bash
~/Documents ls             # home directory
~/.config ls               # hidden config dir
~user/public ls            # another user's directory
```

### Brace Expansion

```bash
# Generate sequences
file{1,2,3}.txt            # file1.txt file2.txt file3.txt
{a,b,c}.rs touch           # create a.rs, b.rs, c.rs

# Ranges
file{1..5}.txt             # file1.txt through file5.txt
{a..z} echo                # a through z
```

---

## Background Jobs

### Running in Background

```bash
# Start command in background
[long-running-task] &
long-running-task &        # traditional also works

# Example
[make -j8] &
```

### Job Control

```bash
# List background jobs
.jobs

# Bring job to foreground
.fg                        # most recent job
.fg 1                      # job number 1

# Send to background
.bg                        # continue stopped job in background
.bg 2                      # specific job

# Wait for all jobs
wait
```

---

## Stack-Native Shell Operations

hsab shell commands return useful values to the stack instead of just printing output.

### File Creation

```bash
# touch returns the created path
newfile.txt touch          # "newfile.txt" on stack
dup cat                    # immediately use it

# mkdir returns the created path
newdir mkdir               # "newdir" on stack
"file.txt" path-join       # "newdir/file.txt"
touch
```

### File Deletion

```bash
# rm returns count of deleted files
*.tmp rm                   # 5 (number deleted)
"Deleted:" swap suffix echo  # "Deleted: 5"
```

### Copying and Moving

```bash
# cp returns destination path
src.txt dst.txt cp         # "dst.txt" on stack

# mv returns destination path
old.txt new.txt mv         # "new.txt" on stack

# Chain operations
src.txt backup/ cp         # "backup/src.txt"
dup cat                    # read the copy
```

### Directory Listing

```bash
# ls returns a vector (list)
*.rs ls                    # ["main.rs", "lib.rs", ...]
spread                     # explode to individual stack items
[-f test] keep             # filter to regular files
[wc -l] each               # count lines in each
```

### Error Handling

All shell operations return `nil` on error:

```bash
nonexistent.txt cat        # nil (file not found)
nil?                       # true
[cat] try                  # safer: catches error
```

---

## Exit Codes

### Checking Last Exit Code

```bash
# Get exit code of last command
$?
exit-code                  # equivalent

# Example
make
$? echo                    # 0 if success, non-zero if failed
```

### Error Checking

```bash
# Check if last command failed
error?                     # true if exit code != 0

# Conditional on success/failure
make
error? ["Build failed" echo] [echo "Build succeeded"] if

# Using try for explicit error handling
[risky-command] try
error? [
  "Error occurred" echo
  drop                     # remove error value
] when
```

### Setting Exit Code

```bash
# Exit with specific code
42 exit

# Exit on error
error? [1 exit] when
```

---

## Key Differences from bash

### Postfix vs Prefix

| bash | hsab |
|------|------|
| `cat file.txt` | `file.txt cat` |
| `grep -r pattern .` | `-r . pattern grep` |
| `mv src dst` | `src dst mv` |

Both styles work in hsab; postfix is preferred when you want stack integration.

### Stack vs Variables

**bash:** Everything goes through variables or pipes

```bash
# bash
result=$(expensive-command)
echo "$result"
echo "$result" | wc -c
```

**hsab:** Results stay on the stack

```bash
# hsab
expensive-command          # result on stack
dup echo                   # use it
len echo                   # use it again
```

### Blocks vs Subshells

**bash:** Subshells create isolated environments

```bash
# bash - subshell
( cd /tmp && make )

# bash - command substitution
files=$(ls *.txt)
```

**hsab:** Blocks are first-class values

```bash
# hsab - blocks are values you can store and pass
[cd /tmp && make] :build-tmp
build-tmp @                # execute it

# hsab - capture output
[ls *.txt] @               # result on stack, not in variable
```

### Conditionals

**bash:**
```bash
if [ -f file.txt ]; then
  echo "exists"
else
  echo "missing"
fi
```

**hsab:**
```bash
file.txt file?
["exists" echo]
["missing" echo]
if
```

Or inline:
```bash
file.txt file? ["exists" echo] ["missing" echo] if
```

### Loops

**bash:**
```bash
for f in *.txt; do
  wc -l "$f"
done
```

**hsab:**
```bash
*.txt ls spread [wc -l] each
```

Or with explicit loop:
```bash
5 [echo "Hello"] times
```

### Quoting

**bash:** Requires careful quoting to avoid word splitting

```bash
# bash - must quote or spaces break things
for f in $(find . -name "*.txt"); do
  process "$f"   # MUST quote $f
done
```

**hsab:** Stack values are discrete items, no word splitting

```bash
# hsab - each filename is one stack entry
*.txt find spread [process] each  # spaces in filenames are fine
```

---

## Quick Reference

| Task | bash | hsab |
|------|------|------|
| List files | `ls -la` | `-la ls` |
| Read file | `cat f.txt` | `f.txt cat` |
| Pipe | `a \| b` | `[a] [b] \|` |
| Redirect out | `cmd > f` | `[cmd] "f" >` |
| Set var | `export V=x` | `"x" "V" setenv` |
| Background | `cmd &` | `[cmd] &` |
| Exit code | `$?` | `$?` or `exit-code` |
| Conditional | `if [ ]; then` | `cond [then] [else] if` |
| Loop files | `for f in *` | `* ls spread [block] each` |
| Subshell | `$(cmd)` | `[cmd] @` |

---

## Common Patterns

### Process files matching pattern

```bash
# bash
for f in *.log; do
  gzip "$f"
done

# hsab
*.log ls spread [gzip] each
```

### Conditional file operations

```bash
# bash
if [ -d "$dir" ]; then
  rm -rf "$dir"
fi

# hsab
$dir dir? [$dir -rf rm] when
```

### Capture and reuse output

```bash
# bash
output=$(complex-pipeline)
echo "$output"
echo "$output" | further-processing

# hsab
complex-pipeline           # on stack
dup echo                   # print it
further-processing         # process it
```

### Error recovery

```bash
# bash
if ! make; then
  echo "Build failed"
  exit 1
fi

# hsab
[make] try
error? ["Build failed" echo 1 exit] when
```
