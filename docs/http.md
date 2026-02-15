# HTTP Client

hsab provides a built-in HTTP client for making web requests. The client uses blocking I/O and automatically parses JSON responses.

## Basic Requests

### GET Request

The simplest form takes just a URL:

```bash
"https://api.example.com/users" fetch
# -> response body (auto-parsed if JSON)
```

### Explicit Method

Specify the HTTP method as a second argument:

```bash
"https://api.example.com/users" "GET" fetch
"https://api.example.com/health" "HEAD" fetch
```

### POST with Body

For requests with a body, put the body first:

```bash
'{"name": "Alice"}' "https://api.example.com/users" "POST" fetch
```

The body is sent with `Content-Type: application/json` by default.

### PUT Request

```bash
'{"name": "Bob", "active": true}' "https://api.example.com/users/123" "PUT" fetch
```

### DELETE Request

```bash
"https://api.example.com/users/123" "DELETE" fetch
```

### PATCH Request

```bash
'{"active": false}' "https://api.example.com/users/123" "PATCH" fetch
```

### Custom Headers

For requests requiring custom headers, pass a Map as the first argument:

```bash
{"Authorization": "Bearer token123", "X-Custom": "value"}
'{"data": 1}'
"https://api.example.com/protected"
"POST"
fetch
```

## Response Handling

### Automatic JSON Parsing

When the response `Content-Type` is `application/json`, hsab automatically parses the body into a native Map or List:

```bash
"https://api.example.com/user/1" fetch
# If response is {"id": 1, "name": "Alice"}
# -> Map: {id: 1, name: "Alice"}

"name" .get
# -> "Alice"
```

Non-JSON responses are returned as strings.

### Status Code Only

Use `fetch-status` to get just the HTTP status code:

```bash
"https://api.example.com/health" fetch-status
# -> 200

"https://api.example.com/missing" fetch-status
# -> 404
```

With explicit method:

```bash
"https://api.example.com/resource" "HEAD" fetch-status
# -> 200
```

### Headers Only

Use `fetch-headers` to get response headers as a Map:

```bash
"https://api.example.com/data" fetch-headers
# -> {"content-type": "application/json", "x-request-id": "abc123", ...}

"content-type" .get
# -> "application/json"
```

## Working with JSON APIs

### Parse and Extract Data

```bash
# Get user and extract name
"https://api.example.com/users/1" fetch
"name" .get
# -> "Alice"

# Get list and count items
"https://api.example.com/users" fetch
count
# -> 42
```

### Send JSON Body

Build JSON from a Map:

```bash
# Create data structure
{"name": "Charlie", "email": "charlie@example.com", "roles": ["user", "admin"]}

# Convert to JSON string and POST
to-json "https://api.example.com/users" "POST" fetch
```

### Extract Nested Data

```bash
"https://api.example.com/nested" fetch
# Response: {"data": {"users": [{"name": "Alice"}, {"name": "Bob"}]}}

"data" .get "users" .get
# -> [{"name": "Alice"}, {"name": "Bob"}]

0 nth "name" .get
# -> "Alice"
```

### Process List Results

```bash
# Get all user names
"https://api.example.com/users" fetch
[["name" .get] map] call
# -> ["Alice", "Bob", "Charlie", ...]
```

## Error Handling

### Network Errors

Network-level errors (DNS failure, connection refused, timeout) raise an `EvalError`:

```bash
["https://unreachable.invalid" fetch] try
error? ["Network error occurred" echo] when
```

### HTTP Errors

HTTP error responses (4xx, 5xx) do not throw errors. Instead, they:

1. Return the response body normally
2. Set the exit code to 1

Check for errors using `error?` after `try`, or check the status code:

```bash
# Method 1: Check exit code after fetch
"https://api.example.com/missing" fetch
$? 0 ne? ["Request failed" echo] when

# Method 2: Check status directly
"https://api.example.com/resource" fetch-status
dup 400 ge? ["Error: " swap suffix echo] when

# Method 3: Use try for complete error handling
[
  "https://api.example.com/data" fetch
  "result" .get
] try
error? [
  "Failed to fetch data" echo
] [
  "Got: " swap suffix echo
] if
```

### Common Status Code Patterns

```bash
# Check for success (2xx)
"https://api.example.com/resource" fetch-status
_status local

$_status 200 ge? $_status 300 lt? and
["Success!"] ["Failed with " $_status suffix] if echo

# Handle specific error codes
$_status 401 eq? ["Unauthorized - check your token"] when
$_status 404 eq? ["Resource not found"] when
$_status 500 ge? ["Server error - try again later"] when
```

## Async HTTP

hsab does not have a dedicated `fetch-async` builtin, but you can make requests asynchronous using the general-purpose `async` operation:

### Single Async Request

```bash
["https://api.example.com/slow" fetch] async
# -> Future

# Do other work...

await
# -> response body
```

### Parallel Requests

Make multiple requests concurrently:

```bash
# Start all requests
["https://api.example.com/users" fetch] async
["https://api.example.com/posts" fetch] async
["https://api.example.com/comments" fetch] async

# Await all three
3 future-await-n
# Stack now has: users posts comments
```

Or using a list:

```bash
[
  ["https://api.example.com/users" fetch]
  ["https://api.example.com/posts" fetch]
  ["https://api.example.com/comments" fetch]
]
[[async] map await-all] call
# -> [users, posts, comments]
```

### Racing Mirrors

Get the fastest response from multiple mirrors:

```bash
[
  ["https://mirror1.example.com/data" fetch]
  ["https://mirror2.example.com/data" fetch]
  ["https://mirror3.example.com/data" fetch]
] race
# -> response from first to complete
```

Or with futures:

```bash
["https://mirror1.example.com/data" fetch] async
["https://mirror2.example.com/data" fetch] async
["https://mirror3.example.com/data" fetch] async
3 collect
future-race
# -> first result
```

### Transform Async Results

Use `future-map` to transform results without awaiting:

```bash
["https://api.example.com/users" fetch] async
[["name" .get] map] future-map
await
# -> list of names
```

## Practical Examples

### REST API CRUD

```bash
# Define API base
"https://api.example.com" _API local

# CREATE
[
  _data local
  $_data to-json "$_API/users" "POST" fetch
] :create-user

# READ
[
  _id local
  "$_API/users/$_id" fetch
] :get-user

# UPDATE
[
  _data local _id local
  $_data to-json "$_API/users/$_id" "PUT" fetch
] :update-user

# DELETE
[
  _id local
  "$_API/users/$_id" "DELETE" fetch
] :delete-user

# Usage
{"name": "Alice", "email": "alice@example.com"} create-user
# -> {"id": 123, "name": "Alice", ...}

123 get-user
# -> {"id": 123, "name": "Alice", ...}

123 {"name": "Alice Smith"} update-user
# -> {"id": 123, "name": "Alice Smith", ...}

123 delete-user
# -> {}
```

### API Client with Authentication

```bash
# Store auth token
"your-api-token" _TOKEN local

# Authenticated fetch helper
[
  _url local
  {"Authorization": "Bearer $_TOKEN"}
  "" $_url "GET" fetch
] :auth-get

[
  _url local _body local
  {"Authorization": "Bearer $_TOKEN"}
  $_body to-json $_url "POST" fetch
] :auth-post

# Usage
"https://api.example.com/me" auth-get
{"title": "New Post"} "https://api.example.com/posts" auth-post
```

### Webhook Handler Script

```bash
#!/usr/bin/env hsab

# Send webhook notification
[
  _event local _data local
  {"event": "$_event", "data": $_data, "timestamp": [date] exec}
  to-json
  "https://webhook.example.com/notify"
  "POST"
  fetch
  drop  # Ignore response
] :notify

# Example: deployment notification
"deploy" {"version": "1.2.3", "env": "production"} notify
```

### Data Pipeline

```bash
# Fetch data from multiple sources and combine
[
  # Parallel fetch
  ["https://api1.example.com/users" fetch] async
  ["https://api2.example.com/profiles" fetch] async
  2 future-await-n

  # Stack: users profiles
  _profiles local _users local

  # Combine data
  $_users [[
    _user local
    $_user "id" .get _id local

    # Find matching profile
    $_profiles [["id" .get $_id eq?] filter] call
    0 nth _profile local

    # Merge
    $_user $_profile merge
  ] map] call
] :fetch-enriched-users

fetch-enriched-users
# -> [{id: 1, name: "Alice", bio: "..."}, ...]
```

### Retry with Backoff

```bash
# Fetch with exponential backoff
[
  _url local
  _attempts local

  $_attempts 1000 mul _delay local  # 1s, 2s, 3s backoff

  [$_url fetch $_? 0 eq?] [
    $_delay delay
    $_attempts 1 plus _attempts local
    $_attempts 1000 mul _delay local
  ] while

  # Return result or fail after max attempts
] :fetch-with-retry

"https://flaky-api.example.com/data" 3 fetch-with-retry
```

### Paginated API

```bash
# Fetch all pages from a paginated API
[
  _base_url local
  [] _results local
  1 _page local
  true _has_more local

  [$_has_more] [
    "$_base_url?page=$_page" fetch
    _response local

    # Append items to results
    $_results $_response "items" .get concat _results local

    # Check for more pages
    $_response "next_page" .get nil? not _has_more local
    $_page 1 plus _page local
  ] while

  $_results
] :fetch-all-pages

"https://api.example.com/items" fetch-all-pages
# -> [all items across all pages]
```

### Health Check Script

```bash
#!/usr/bin/env hsab

# Check multiple endpoints
[
  ["https://api.example.com/health" "https://db.example.com/ping" "https://cache.example.com/status"]
  [[
    _url local
    [$_url fetch-status 200 eq?]
    ["OK: $_url"]
    ["FAIL: $_url"]
    if echo
  ] each] call
] :health-check

health-check
```

## Supported HTTP Methods

The fetch operations support these HTTP methods:

| Method | Description |
|--------|-------------|
| `GET` | Retrieve data (default) |
| `POST` | Create new resource |
| `PUT` | Replace resource |
| `PATCH` | Partial update |
| `DELETE` | Remove resource |
| `HEAD` | Get headers only |
| `OPTIONS` | Get allowed methods |

Custom methods are also supported by passing any string as the method argument.

## Quick Reference

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `fetch` | `url -- response` | GET request |
| `fetch` | `url method -- response` | Request with method |
| `fetch` | `body url method -- response` | Request with body |
| `fetch` | `headers body url method -- response` | Request with headers |
| `fetch-status` | `url -- status` | Get status code |
| `fetch-status` | `url method -- status` | Get status code with method |
| `fetch-headers` | `url -- headers` | Get response headers |
| `fetch-headers` | `url method -- headers` | Get headers with method |
