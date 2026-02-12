# Bash to hsab Migration Guide

This guide helps bash users transition to hsab's postfix stack-based syntax. Each pattern shows the bash equivalent alongside the hsab version.

## Table of Contents
1. [Core Concept: Postfix vs Prefix](#core-concept)
2. [Basic Commands](#basic-commands)
3. [Variables](#variables)
4. [Pipes and Redirects](#pipes-and-redirects)
5. [Conditionals](#conditionals)
6. [Loops](#loops)
7. [Functions](#functions)
8. [File Operations](#file-operations)
9. [String Manipulation](#string-manipulation)
10. [Command Substitution](#command-substitution)

---

## Core Concept: Postfix vs Prefix {#core-concept}

The key difference: in bash, commands come first followed by arguments. In hsab, arguments come first (pushed to stack), then the command pops them.

```bash
# Bash: command arg1 arg2
echo hello world
cp source.txt dest.txt
```

```hsab
# hsab: arg2 arg1 command (LIFO - last in, first out)
world hello echo
dest.txt source.txt cp
```

**Mental model**: Think of hsab like a calculator with RPN (Reverse Polish Notation). You enter values, then operate on them.

---

## Basic Commands {#basic-commands}

### 1. Simple command execution

```bash
# Bash
ls -la
grep "pattern" file.txt
```

```hsab
# hsab
-la ls
file.txt "pattern" grep
```

### 2. Multiple commands (sequential)

```bash
# Bash
pwd; ls; date
```

```hsab
# hsab (semicolons optional, each runs independently)
pwd; ls; date
# or
pwd
ls
date
```

### 3. Current directory listing with options

```bash
# Bash
ls -la /tmp
```

```hsab
# hsab
/tmp -la ls
```

---

## Variables {#variables}

### 4. Setting and using variables

```bash
# Bash
NAME="Alice"
echo "Hello, $NAME"
```

```hsab
# hsab (uses environment variables)
Alice NAME export
"Hello, $NAME" echo
```

### 5. Variable in commands

```bash
# Bash
FILE=/etc/passwd
cat $FILE
```

```hsab
# hsab
/etc/passwd FILE export
$FILE cat
```

### 6. String interpolation in double quotes

```bash
# Bash
echo "User: $USER, Home: $HOME"
```

```hsab
# hsab (same syntax in double quotes)
"User: $USER, Home: $HOME" echo
```

---

## Pipes and Redirects {#pipes-and-redirects}

### 7. Simple pipe

```bash
# Bash
ls | grep txt
```

```hsab
# hsab: [producer] [consumer] |
ls [grep txt] |
```

### 8. Multi-stage pipeline

```bash
# Bash
cat file.txt | grep error | sort | uniq
```

```hsab
# hsab: chain pipes
file.txt cat [grep error] | [sort] | [uniq] |
```

### 9. Redirect stdout to file

```bash
# Bash
echo "hello" > output.txt
```

```hsab
# hsab: [command] [file] >
[hello echo] [output.txt] >
```

### 10. Append to file

```bash
# Bash
echo "line" >> log.txt
```

```hsab
# hsab
[line echo] [log.txt] >>
```

### 11. Redirect stderr

```bash
# Bash
command 2> errors.txt
```

```hsab
# hsab
[command] [errors.txt] 2>
```

---

## Conditionals {#conditionals}

### 12. If-then-else

```bash
# Bash
if [ -f file.txt ]; then
    echo "exists"
else
    echo "not found"
fi
```

```hsab
# hsab: [condition] [then] [else] if
[file.txt file?] [exists echo] [not\ found echo] if
```

### 13. String comparison

```bash
# Bash
if [ "$a" = "$b" ]; then
    echo "equal"
fi
```

```hsab
# hsab
[$a $b eq?] [equal echo] [] if
```

### 14. Numeric comparison

```bash
# Bash
if [ $x -gt 10 ]; then
    echo "big"
fi
```

```hsab
# hsab
[$x 10 gt?] [big echo] [] if
```

### 15. And/Or logic

```bash
# Bash
cmd1 && cmd2
cmd1 || cmd2
```

```hsab
# hsab
[cmd1] [cmd2] &&
[cmd1] [cmd2] ||
```

---

## Loops {#loops}

### 16. For loop (fixed count)

```bash
# Bash
for i in 1 2 3 4 5; do
    echo $i
done
```

```hsab
# hsab: N [body] times
5 [echo] times
```

### 17. While loop

```bash
# Bash
while [ condition ]; do
    body
done
```

```hsab
# hsab: [condition] [body] while
[condition] [body] while
```

### 18. Loop with break

```bash
# Bash
while true; do
    if [ condition ]; then break; fi
    body
done
```

```hsab
# hsab
[true] [[condition] [break] [] if; body] while
```

### 19. Process list of files

```bash
# Bash
for f in *.txt; do
    echo "Processing $f"
done
```

```hsab
# hsab: glob, spread, then each
*.txt spread ["Processing: " swap suffix echo] each
```

---

## Functions {#functions}

### 20. Define and call a function

```bash
# Bash
greet() {
    echo "Hello, $1"
}
greet "World"
```

```hsab
# hsab: [body] :name defines a word
["Hello, " swap suffix echo] :greet
World greet
```

### 21. Function with multiple operations

```bash
# Bash
backup() {
    cp "$1" "$1.bak"
}
backup important.txt
```

```hsab
# hsab
[dup .bak suffix cp] :backup
important.txt backup
```

---

## File Operations {#file-operations}

### 22. Copy with path manipulation

```bash
# Bash
cp file.txt /backup/file.txt
```

```hsab
# hsab: use path-join
file.txt dup /backup swap path-join cp
# or more simply
/backup/file.txt file.txt cp
```

### 23. Check file exists

```bash
# Bash
if [ -f myfile ]; then echo "yes"; fi
```

```hsab
# hsab
[myfile file?] [yes echo] [] if
```

### 24. Get basename/dirname

```bash
# Bash
basename /path/to/file.txt
dirname /path/to/file.txt
```

```hsab
# hsab
/path/to/file.txt basename
/path/to/file.txt dirname
```

---

## String Manipulation {#string-manipulation}

### 25. Add suffix/prefix

```bash
# Bash
echo "${file}_backup"
```

```hsab
# hsab
file _backup suffix echo
# or
$file _backup suffix echo
```

### 26. Split string

```bash
# Bash
echo "a:b:c" | cut -d: -f1
```

```hsab
# hsab: split1 splits on first occurrence
"a:b:c" ":" split1 drop echo  # prints "a"
```

---

## Command Substitution {#command-substitution}

### 27. Use command output as argument

```bash
# Bash
ls $(pwd)
echo "Today is $(date)"
```

```hsab
# hsab: command output stays on stack, consumed by next command
pwd ls
# For string interpolation:
date ["Today is " swap suffix echo] |
```

### 28. Capture output for later use

```bash
# Bash
RESULT=$(some-command)
echo $RESULT
```

```hsab
# hsab: stack holds values between commands
some-command dup echo  # keeps value on stack
# or use a variable:
some-command RESULT export; $RESULT echo
```

---

## Quick Reference Table

| Bash | hsab | Description |
|------|------|-------------|
| `echo hello` | `hello echo` | Simple command |
| `cmd1 \| cmd2` | `cmd1 [cmd2] \|` | Pipe |
| `cmd > file` | `[cmd] [file] >` | Redirect |
| `if [...]; then A; else B; fi` | `[...] [A] [B] if` | Conditional |
| `for i in ...; do ...; done` | `... spread [...] each` | Loop over items |
| `while [...]; do ...; done` | `[...] [...] while` | While loop |
| `func() { ... }` | `[...] :func` | Define function |
| `$(cmd)` | `cmd` (on stack) | Command substitution |
| `$VAR` | `$VAR` | Variable expansion |
| `"...$VAR..."` | `"...$VAR..."` | String interpolation |

---

## Tips for Transition

1. **Think "data first, action last"**: Push what you need, then operate
2. **Stack is your friend**: Values stay on stack until consumed
3. **Blocks defer execution**: `[cmd]` doesn't run until applied with `@` or operators
4. **Use `.stack` in REPL**: See what's on the stack at any time
5. **Use `.debug` for learning**: Step through expressions to understand flow
6. **Start simple**: Convert one command at a time, test interactively

---

## Common Gotchas

1. **Argument order is reversed** (LIFO): `dest src cp` not `src dest cp`
2. **Pipes need blocks**: `ls [grep txt] |` not `ls | grep txt`
3. **Conditions need blocks**: `[test] [then] [else] if`
4. **Semicolons separate lines**, not required at end
5. **Quotes preserve spaces**: `"hello world"` is one value, `hello world` is two
