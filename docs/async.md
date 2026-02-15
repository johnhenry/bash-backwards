# Async and Futures in hsab

hsab provides a comprehensive async/futures system for background execution, parallel processing, and concurrent operations.

## Futures Concept

A **Future** represents a computation running in the background. When you launch an async operation:

1. The operation starts immediately in a separate thread
2. You get back a **Future handle** you can hold, pass around, or store
3. The main shell remains responsive while the operation runs
4. You can check status, await completion, or cancel at any time

Futures are **non-blocking** by default. This means you can fire off multiple operations and continue working while they complete.

```bash
# Start a long download in the background
[curl -s https://large-file.example.com/data.zip -o data.zip] async
# Returns immediately with a Future handle on the stack

# Continue doing other work...
*.txt ls spread

# Later, await the result when you need it
await
```

## Basic Async Operations

### Creating Futures

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `[block] async` | `->` Future | Run block in background, return Future |
| `ms delay-async` | `->` Future | Return Future that resolves after delay |

```bash
# Start background task
[heavy-computation] async     # -> Future

# Multiple async operations
[task1] async                 # -> Future1
[task2] async                 # -> Future1 Future2
```

### Awaiting Results

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `Future await` | `->` result | Block until complete, push result |
| `Future future-result` | `->` Map | Non-throwing: `{ok: v}` or `{err: e}` |

```bash
# Simple await
[curl -s api.example.com] async
await                         # Blocks until done, pushes response

# Non-throwing await (for error handling)
[risky-operation] async
future-result                 # -> {ok: value} or {err: message}
"ok" get                      # Extract success value
```

### Checking Status

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `Future future-status` | Future `->` Future String | Check without consuming: "pending", "completed", "failed", "cancelled" |

```bash
[long-task] async
future-status echo            # "pending"
# ... do other work ...
future-status echo            # "completed"
await                         # Get the result
```

Note: `future-status` is non-consuming. The Future stays on the stack so you can check it multiple times.

### Cancellation

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `Future future-cancel` | `->` | Cancel a pending Future |

```bash
[slow-download] async
# Changed my mind
future-cancel                 # Marks as cancelled

# Awaiting a cancelled future throws an error
# Use future-result for graceful handling:
[slow-download] async
dup future-cancel
future-result                 # -> {err: "cancelled"}
```

## Parallel Execution

For running multiple independent operations concurrently.

### Run All, Wait for All

```bash
# parallel: Run all blocks concurrently, collect all results
[[blocks]] parallel -> [results]
```

```bash
# Check multiple servers at once
[
  [api.example.com ping]
  [db.example.com ping]
  [cache.example.com ping]
] parallel
# -> ["64 bytes from...", "64 bytes from...", "64 bytes from..."]
```

### Concurrency-Limited Parallel

```bash
# parallel-n: Run with at most N concurrent operations
[[blocks]] N parallel-n -> [results]
```

```bash
# Process 100 files, but only 4 at a time
*.json ls spread
[[process-json] each] collect
4 parallel-n                  # Rate-limited parallel processing
```

### Race: First to Complete

```bash
# race: Return first successful result, cancel others
[[blocks]] race -> result
```

```bash
# Try multiple mirrors, use fastest
[
  [https://mirror1.example.com/file.tar.gz curl -s]
  [https://mirror2.example.com/file.tar.gz curl -s]
  [https://mirror3.example.com/file.tar.gz curl -s]
] race
# -> Response from whichever mirror responded first
```

## Future Combinators

Operations for working with collections of futures.

### Await Multiple Futures from Stack

```bash
# future-await-n: Await N futures from stack, push results
future1 future2 ... futureN N future-await-n -> result1 result2 ... resultN
```

```bash
[task-a] async
[task-b] async
[task-c] async
3 future-await-n              # -> resultA resultB resultC
```

### Await a List of Futures

```bash
# await-all: Await all futures in a list
[futures] await-all -> [results]
```

```bash
# Build a list of futures first
marker
[task1] async
[task2] async
[task3] async
collect                       # -> [Future1, Future2, Future3]
await-all                     # -> [result1, result2, result3]
```

### Race a List of Futures

```bash
# future-race: Race futures, return first result
[futures] future-race -> result
```

```bash
# Create futures, then race them
marker
[slow-api] async
[fast-api] async
collect
future-race                   # -> First result
```

### Transform Without Awaiting

```bash
# future-map: Transform the eventual result
Future [block] future-map -> Future
```

```bash
# Chain transformations on future results
[fetch-data] async
[json "items" get] future-map
[len] future-map
await                         # -> Number of items (computed lazily)
```

`future-map` returns a new Future that will apply the transformation when the original completes. The block doesn't run until you await.

## Timeout and Retry

### Timeout

```bash
# timeout: Kill operation if it exceeds time limit
[block] seconds timeout
```

```bash
# Give up after 5 seconds
[curl -s slow-api.example.com] 5 timeout
# Sets exit code 124 if timed out (standard timeout exit code)
```

### Retry on Failure

```bash
# retry: Retry N times until success
[block] N retry -> result
```

```bash
# Retry flaky API up to 3 times
[curl -s flaky-api.example.com] 3 retry
```

### Retry with Delay

```bash
# retry-delay: Retry with delay between attempts
[block] N ms retry-delay -> result
```

```bash
# Retry 5 times with 1-second delay between attempts
[curl -s rate-limited-api.example.com] 5 1000 retry-delay
```

The delay helps with rate-limited APIs or transient failures.

## Error Handling

### Non-Throwing Result Access

Use `future-result` for graceful error handling:

```bash
[risky-operation] async
future-result
# -> {ok: value} on success
# -> {err: message} on failure

# Pattern: check and branch
dup "ok" get nil eq?
[
  "err" get "Failed: " swap suffix echo
]
[
  "ok" get process-success
] if
```

### Errors in Async Blocks

When a block throws an error during async execution:

1. The error is captured
2. The Future moves to "failed" state
3. `await` will re-throw the error
4. `future-result` returns `{err: "..."}`

```bash
[throw "something went wrong"] async
await                         # Throws: "future failed: something went wrong"

# Safe alternative:
[throw "something went wrong"] async
future-result                 # -> {err: "EvalError: something went wrong"}
```

## Visual Feedback

### Prompt Shows Pending Count

When futures are pending, the prompt displays them:

```
hsab [2]>                     # 2 pending futures
```

Customize this in your PS1 definition:

```bash
[
  "hsab"
  [$_FUTURES "0" gt?] [" [" $_FUTURES "]" suffix suffix suffix] [] if
  "> " suffix
] :PS1
```

### The _FUTURES Variable

The `$_FUTURES` context variable contains the count of pending futures:

```bash
$_FUTURES echo                # "2"
```

This updates automatically as futures complete or are cancelled.

## Practical Examples

### Parallel API Calls

Fetch data from multiple endpoints simultaneously:

```bash
# Define endpoints
marker
"https://api.example.com/users"
"https://api.example.com/posts"
"https://api.example.com/comments"
collect

# Fetch all in parallel
[[curl -s] map] @
parallel

# Process results
[json] each
```

### Racing Mirrors

Download from the fastest available mirror:

```bash
# Race multiple download sources
[
  [https://us.mirror.example.com/pkg.tar.gz curl -sL]
  [https://eu.mirror.example.com/pkg.tar.gz curl -sL]
  [https://asia.mirror.example.com/pkg.tar.gz curl -sL]
] race

# Save the winning response
"pkg.tar.gz" save
```

### Background Downloads with Progress Check

Start downloads and check on them periodically:

```bash
# Start multiple downloads
[curl -s https://example.com/file1.zip -o file1.zip] async :dl1
[curl -s https://example.com/file2.zip -o file2.zip] async :dl2
[curl -s https://example.com/file3.zip -o file3.zip] async :dl3

# Check status
dl1 future-status echo        # "pending"
dl2 future-status echo        # "completed"
dl3 future-status echo        # "pending"

# Wait for all when ready
dl1 dl2 dl3 3 future-await-n
```

### Batch Processing with Rate Limiting

Process many items without overwhelming the system:

```bash
# Get list of URLs to process
urls.txt lines spread

# Create blocks for each URL
[[curl -s] suffix [| process] suffix] each

# Run at most 10 at a time
collect 10 parallel-n

# Results now on stack
```

### Resilient Network Operations

Handle unreliable network with retry and timeout:

```bash
# Retry with exponential backoff style (multiple retry-delay calls)
[
  [
    [https://flaky-api.example.com curl -s] 3 100 retry-delay
  ] 10 timeout
] try

error?
["API unavailable after retries" echo]
[json process]
if
```

### Async Map-Reduce Pattern

Process items asynchronously, then aggregate:

```bash
# Fetch data from multiple sources
[
  [source1.example.com fetch json "count" get]
  [source2.example.com fetch json "count" get]
  [source3.example.com fetch json "count" get]
] parallel

# Sum the counts
sum
"Total count: " swap suffix echo
```

## Summary

| Operation | Description |
|-----------|-------------|
| `[block] async` | Start background execution |
| `Future await` | Block until complete |
| `Future future-status` | Check status (non-consuming) |
| `Future future-cancel` | Cancel pending future |
| `Future future-result` | Get result as `{ok:...}` or `{err:...}` |
| `[[blocks]] parallel` | Run all, collect all results |
| `[[blocks]] N parallel-n` | Run with concurrency limit |
| `[[blocks]] race` | First to complete wins |
| `N future-await-n` | Await N futures from stack |
| `[futures] await-all` | Await list of futures |
| `[futures] future-race` | Race list of futures |
| `Future [block] future-map` | Transform without awaiting |
| `[block] sec timeout` | Kill if exceeds time |
| `[block] N retry` | Retry on failure |
| `[block] N ms retry-delay` | Retry with delay |
