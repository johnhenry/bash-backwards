# Shell Migration Guide: bash/zsh to hsab

This guide helps bash and zsh users transition to hsab. Most shell knowledge transfers directly; the main difference is postfix notation and the persistent stack.

## Table of Contents

1. [Running Commands](#running-commands)
2. [Pipelines and Redirection](#pipelines-and-redirection)
3. [Variables and Environment](#variables-and-environment)
4. [Conditionals and Loops](#conditionals-and-loops)
5. [Functions and Aliases](#functions-and-aliases)
6. [Job Control](#job-control)
7. [Stack-Native Shell Operations](#stack-native-shell-operations)
8. [Exit Codes](#exit-codes)
9. [Key Differences from bash](#key-differences-from-bash)

**Related documentation:**
- [Getting Started](getting-started.md) - Introduction to hsab
- [Reference: File Operations](reference.md#file-operations) - Complete operation reference
- [Configuration Guide](config.md) - Environment and REPL settings

---

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

hsab reimagines shell operations as **composable, value-returning functions**. Instead of side-effect-only commands, operations return useful values to the stack, enabling functional pipelines.

**Key principle:** Every operation returns a value. Errors return `nil`.

See also: [Reference: File Operations](reference.md#file-operations)

### Why Stack-Native?

Traditional shells:
```bash
# bash: side effects only, result disappears
touch file.txt
mkdir project
cp src.txt dst.txt
```

hsab stack-native:
```bash
# hsab: operations return values for chaining
file.txt touch             # Returns "/abs/path/file.txt"
dup cat                    # Use the returned path immediately

project mkdir              # Returns path
"src" path-join mkdir      # Chain: create project/src
```

### Complete Operation Reference

| Operation | Stack Effect | Returns | On Error |
|-----------|--------------|---------|----------|
| `touch` | `path -- path` | Canonical absolute path | `nil` |
| `mkdir` | `path -- path` | Created directory path | `nil` |
| `mkdir-p` | `path -- path` | Created directory path | `nil` |
| `mktemp` | `-- path` | Temp file path | (always succeeds) |
| `mktemp-d` | `-- path` | Temp directory path | (always succeeds) |
| `cp` | `src dst -- dst` | Destination path | `nil` |
| `mv` | `src dst -- dst` | Destination path | `nil` |
| `rm` | `path -- count` | Number of files deleted | `nil` |
| `rm-r` | `path -- count` | Number of items deleted | `nil` |
| `ln` | `target link -- link` | Link path | `nil` |
| `realpath` | `path -- path` | Canonical absolute path | `nil` |
| `cd` | `[path] -- path` | New working directory | `nil` |
| `which` | `cmd -- path` | Executable path | `nil` |
| `extname` | `path -- ext` | File extension | `""` |
| `ls` | `[pattern] -- [files]` | Vector of filenames | `[]` |
| `glob` | `pattern -- [paths]` | Vector of matches | `[]` |

### File Creation Operations

#### touch: Create Files

```bash
# Create file, get canonical path back
"newfile.txt" touch        # "/home/user/project/newfile.txt"

# Immediately use the returned path
"config.json" touch dup    # Create and keep path
"{}" swap write            # Write empty JSON object

# Error handling
"/no/such/dir/file.txt" touch  # nil (parent doesn't exist)
nil? [
    "Failed to create file" echo
] [] if
```

#### mkdir / mkdir-p: Create Directories

```bash
# Create single directory
"logs" mkdir               # "/home/user/project/logs"

# Create nested directories
"src/components/ui" mkdir-p   # Creates all parents

# Chain directory creation
"project" mkdir            # Returns "/abs/path/project"
"src" path-join mkdir      # Returns "/abs/path/project/src"
"lib" path-join mkdir      # Returns "/abs/path/project/src/lib"
```

#### mktemp / mktemp-d: Temporary Files

```bash
# Create temp file with unique name
mktemp                     # "/tmp/hsab-a1b2c3"
"scratch data" swap write  # Write to it

# Create temp directory
mktemp-d                   # "/tmp/hsab-dir-x7y8z9"
dup "file1.txt" path-join touch
swap "file2.txt" path-join touch

# Pattern: temp workspace
mktemp-d dup cd            # Create and enter temp dir
# ... do work ...
drop cd                    # Return to previous dir
```

### File Operations

#### cp: Copy Files

```bash
# Copy file, get destination path
"original.txt" "backup.txt" cp     # "backup.txt"

# Copy to directory
"report.pdf" "archive/" cp         # "archive/report.pdf"

# Chain: copy and process
"data.csv" "working.csv" cp        # Returns destination
cat                                # Read the copy
json                               # Parse it

# Batch copy with returned paths
*.txt ls spread                    # All .txt files
[dup "backups/" swap basename path-join cp] each
collect                            # Vector of backup paths
```

#### mv: Move/Rename Files

```bash
# Rename file
"old-name.txt" "new-name.txt" mv   # "new-name.txt"

# Move to directory
"file.txt" "/archive/" mv          # "/archive/file.txt"

# Rename with transformation
"photo.jpeg" dup
".jpg" reext mv                    # "photo.jpg"

# Batch rename
*.JPEG ls spread
[dup ".jpg" reext mv] each         # Rename all to .jpg
```

#### rm / rm-r: Remove Files

```bash
# Remove single file, get count
"temp.txt" rm                      # 1

# Remove multiple via glob
"*.log" rm                         # 7 (count of deleted)

# Display result
*.tmp rm                           # 5
"Cleaned up" swap " files" suffix suffix echo
# "Cleaned up 5 files"

# Recursive removal
"old-build/" rm-r                  # 142 (total items)

# Safe removal with check
"file.txt" dup -f test             # Check exists first
[rm "Removed" echo] [] if
```

#### ln: Create Symlinks

```bash
# Create symbolic link
"/usr/local/bin/python3" "python" ln   # "python"

# Relative symlink
"../shared/config.json" "config.json" ln

# Link and verify
"/data/large-file" "local-link" ln
realpath                           # Shows target
```

### Path Operations

#### cd: Change Directory

```bash
# Change to directory, get new path
"/tmp" cd                          # "/tmp"

# Home directory (no argument)
cd                                 # "/home/user"

# Tilde expansion
"~/Documents" cd                   # "/home/user/Documents"

# Navigate and return
pwd                                # Save current
"/tmp" cd                          # Go to /tmp
# ... do work ...
cd                                 # Back to saved (home)

# Error handling
"/nonexistent" cd                  # nil
nil? ["Directory not found" echo] [] if
```

#### realpath: Resolve Paths

```bash
# Resolve relative path
"../sibling/file.txt" realpath     # "/home/user/sibling/file.txt"

# Resolve symlink
"link" realpath                    # Target path

# Resolve tilde
"~/Documents" realpath             # "/home/user/Documents"

# Non-existent returns nil
"/no/such/path" realpath           # nil
```

#### extname: Get Extension

```bash
# Extract extension
"file.txt" extname                 # ".txt"
"archive.tar.gz" extname           # ".gz" (outermost)
"Makefile" extname                 # "" (no extension)

# Use in processing
"data.json" dup extname
".json" eq? [json cat] [] if       # Parse if JSON
```

### Directory Listing

#### ls: List Directory

```bash
# List current directory (returns vector)
ls                                 # ["file1.txt", "dir/", ...]

# List specific directory
"/tmp" ls                          # ["temp1", "temp2", ...]

# With glob pattern
"*.rs" ls                          # ["main.rs", "lib.rs"]

# Process listing
"src/" ls spread                   # Explode to stack
[-f test] keep                     # Filter to files
[wc -l] each                       # Count lines each
collect                            # Gather results

# Count files by extension
ls spread
[extname] each
collect sort uniq -c               # Count by extension
```

#### glob: Pattern Matching

```bash
# Simple glob
"*.txt" glob                       # ["a.txt", "b.txt"]

# Recursive glob
"**/*.rs" glob                     # All .rs files, any depth

# Multiple patterns
"{*.rs,*.toml}" glob               # .rs and .toml files

# Process matches
"**/*.md" glob spread
[wc -w] each                       # Word count each
sum                                # Total words
```

#### which: Find Executables

```bash
# Find command
"python3" which                    # "/usr/bin/python3"
"cargo" which                      # "/home/user/.cargo/bin/cargo"

# Not found returns nil
"nonexistent" which                # nil
nil? ["Command not found" echo] [] if

# Use in scripts
"node" which nil?
["nodejs" which] [] if             # Fallback to nodejs
```

### Error Handling Patterns

All operations return `nil` on error, enabling clean error handling:

```bash
# Pattern 1: Check with nil?
"file.txt" touch
nil? [
    "Failed to create file" echo
    1 exit
] [] if

# Pattern 2: Default value
"config.json" cat
nil? drop ["{}" json] [] if        # Use default if missing

# Pattern 3: Early return
"/important" cd
nil? ["Cannot access directory" echo 1 exit] [] if

# Pattern 4: Conditional execution
"src.txt" "dst.txt" cp
nil? [] [
    "Copied successfully" echo
    cat                            # Process the copy
] if

# Pattern 5: Try for complex operations
[
    "data.json" cat json
    "items" get spread
    [process-item] each
] try
error? ["Processing failed" echo] [] if
```

### Compositional Pipelines

Stack-native operations shine in pipelines:

```bash
# Create project structure
"myproject" mkdir dup
"src" path-join mkdir drop
"tests" path-join mkdir drop
"docs" path-join mkdir drop
"Created project:" swap suffix echo

# Process all source files
"src/**/*.rs" glob spread
[dup wc -l swap basename " lines in " swap suffix suffix echo] each

# Backup and transform
"data/" ls spread
[
    dup "backup/" swap basename path-join cp  # Copy to backup
    dup cat json                               # Read original
    "processed" true set                       # Add field
    to-json swap write                         # Write back
] each

# Find large files
ls spread
[dup stat "size" get 1000000 gt?] keep        # Filter >1MB
[dup stat "size" get ":" swap suffix suffix echo] each

# Clean build artifacts
"target/" -d test
[
    "target/" rm-r
    "Cleaned" swap " items" suffix suffix echo
] [] if
```

### Comparison: bash vs hsab

| Task | bash | hsab |
|------|------|------|
| Create and use file | `touch f.txt && cat f.txt` | `f.txt touch cat` |
| Copy and read | `cp a b && cat b` | `a b cp cat` |
| Count deleted | `rm *.tmp` (no count) | `*.tmp rm echo` |
| Create nested dirs | `mkdir -p a/b/c` | `a/b/c mkdir-p` |
| Check if created | `mkdir d && echo ok` | `d mkdir nil? [] [ok echo] if` |
| Find or default | `which python \|\| which python3` | `python which nil? [python3 which] [] if` |

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
