# Terminal Graphics and Media Handling

hsab supports inline images, clickable hyperlinks, and clipboard operations using modern terminal escape sequences. This guide covers the `Value::Media` and `Value::Link` types, and terminal protocol support.

## Media Type

The `Value::Media` type represents image data with metadata:

```rust
Value::Media {
    mime_type: String,      // e.g., "image/png", "image/jpeg"
    data: Vec<u8>,          // Raw binary data
    width: Option<u32>,     // Width in pixels (if known)
    height: Option<u32>,    // Height in pixels (if known)
    alt: Option<String>,    // Alt text / description
    source: Option<String>, // Original file path or URL
}
```

### Supported Formats

| Format | MIME Type | Dimensions Detected |
|--------|-----------|---------------------|
| PNG | `image/png` | Yes |
| JPEG | `image/jpeg` | No (planned) |
| GIF | `image/gif` | Yes |
| WebP | `image/webp` | No |
| SVG | `image/svg+xml` | No |
| BMP | `image/bmp` | No |
| TIFF | `image/tiff` | No |
| ICO | `image/x-icon` | No |

## Loading and Displaying Images

### image-load

Load an image file from disk:

```bash
# Load an image onto the stack
"~/Photos/screenshot.png" image-load

# Load and immediately display
"chart.png" image-load image-show
```

The path supports tilde expansion (`~` for home directory).

### image-show

Display a Media value in the terminal:

```bash
"logo.png" image-load image-show
```

The display method depends on your terminal's capabilities (see Terminal Protocol Support below). The image-show operation is non-destructive: the Media value remains on the stack after display.

### image-info

Get metadata about a Media value as a record:

```bash
"photo.png" image-load image-info
# => {mime_type: "image/png", size: 45678, width: 800, height: 600, source: "photo.png"}
```

Returned fields:
- `mime_type` - MIME type string
- `size` - Size in bytes
- `width` - Width in pixels (if detected)
- `height` - Height in pixels (if detected)
- `alt` - Alt text (if set)
- `source` - Original file path (if loaded from file)

## Terminal Protocol Support

hsab automatically detects which graphics protocol your terminal supports and uses the best available option.

### iTerm2 (OSC 1337)

Used when `TERM_PROGRAM` contains "iterm". This is the primary protocol for iTerm2 and compatible terminals on macOS.

```
ESC ] 1337 ; File = inline=1;size=N;width=W;height=H : BASE64_DATA BEL
```

Features:
- Inline image display
- Automatic aspect ratio preservation
- Size in terminal cells

### Kitty Graphics Protocol

Used when `KITTY_WINDOW_ID` is set. Kitty uses APC (Application Program Command) sequences.

```
ESC _ G a=T,f=100,m=0 ; BASE64_DATA ESC \
```

Features:
- High-quality PNG rendering
- Chunked transmission for large images (>4KB)
- Multiple format support

### Sixel

Detected via `TERM` containing "sixel", "mlterm", or "mintty". Sixel is a bitmap graphics format supported by many terminals.

Note: Sixel rendering is planned but not yet implemented. Images will display as placeholders.

### Fallback (Text Placeholder)

When no graphics protocol is detected, images display as descriptive text:

```
[Image: image/png 800x600 45.2 KB (photo.png)]
```

### Checking Terminal Support

To see which protocol hsab detected:

```bash
# The protocol is auto-detected and cached
# Check by loading and showing an image
"test.png" image-load image-show
```

## Base64 Encoding

Media values can be converted to and from base64 strings.

### to-base64

Convert Media (or any binary data) to a base64 string:

```bash
"image.png" image-load to-base64
# => "iVBORw0KGgoAAAANSUhEUgAAA..."
```

Also works with Bytes and strings:

```bash
"hello world" to-base64
# => "aGVsbG8gd29ybGQ="
```

### from-base64

Convert a base64 string back to Bytes:

```bash
"aGVsbG8gd29ybGQ=" from-base64
# => Bytes (11 bytes)

# Convert to string
"aGVsbG8gd29ybGQ=" from-base64 to-string
# => "hello world"
```

Note: `from-base64` produces `Value::Bytes`, not `Value::Media`. To create a Media value from base64 data, you would need to construct it with the appropriate MIME type.

## Hyperlinks (OSC 8)

The `Value::Link` type creates clickable hyperlinks in supported terminals.

### Creating Links

```bash
# URL-only link (displays the URL as clickable text)
"https://example.com" link

# Link with custom display text
"Click here" "https://example.com" link
```

### link-info

Get information about a Link value:

```bash
"Visit site" "https://example.com" link link-info
# => {url: "https://example.com", text: "Visit site"}
```

### Terminal Support

Links use OSC 8 escape sequences:

```
ESC ] 8 ; ; URL BEL TEXT ESC ] 8 ; ; BEL
```

Supported in:
- iTerm2
- Kitty
- GNOME Terminal (VTE-based)
- Windows Terminal
- Alacritty
- Many modern terminals

In unsupported terminals, the display text (or URL) shows as plain text.

## Clipboard Operations (OSC 52)

Copy and paste using terminal escape sequences.

### clip-copy

Copy a value to the system clipboard (non-destructive):

```bash
"Hello, clipboard!" clip-copy
# Value remains on stack, text copied to clipboard
```

Works with any value that has a string representation.

### clip-cut

Copy to clipboard and remove from stack (destructive):

```bash
"Temporary value" clip-cut
# Stack is now empty, text is in clipboard
```

### clip-paste

Paste from the system clipboard:

```bash
clip-paste
# => contents of clipboard as a string
```

Note: `clip-paste` requires terminal support for OSC 52 queries and may timeout (500ms) if unsupported.

### How It Works

OSC 52 sends clipboard data to the terminal:

```
ESC ] 52 ; c ; BASE64_DATA BEL
```

Supported in:
- iTerm2 (with "Allow clipboard access" enabled)
- Kitty
- tmux (with `set-clipboard on`)
- Some SSH clients (via terminal passthrough)

## Terminal Support Matrix

| Feature | iTerm2 | Kitty | GNOME/VTE | Windows Terminal | xterm |
|---------|--------|-------|-----------|------------------|-------|
| Inline Images | Yes (OSC 1337) | Yes (APC) | Sixel* | Sixel* | Sixel* |
| Hyperlinks | Yes | Yes | Yes | Yes | No |
| Clipboard Copy | Yes | Yes | No | No | Yes |
| Clipboard Paste | Yes | Yes | No | No | Limited |

*Sixel support varies by version and configuration.

### Environment Variables Checked

| Variable | Protocol |
|----------|----------|
| `TERM_PROGRAM=iTerm.app` | iTerm2 |
| `KITTY_WINDOW_ID` | Kitty |
| `TERM` contains "sixel" | Sixel |
| `TERM` contains "mlterm" | Sixel |
| `TERM` contains "mintty" | Sixel |

### Enabling Terminal Features

**iTerm2:**
1. Preferences > General > Selection
2. Enable "Applications in terminal may access clipboard"

**Kitty:**
Enabled by default. Configure in `kitty.conf`:
```
clipboard_control write-clipboard write-primary read-clipboard read-primary
```

**tmux:**
Add to `~/.tmux.conf`:
```
set -g set-clipboard on
```

## Example Workflows

### Display an Image

```bash
# Simple display
"diagram.png" image-load image-show

# Load, check dimensions, display
"photo.png" image-load dup image-info "width" get echo image-show
```

### Create a Linked Image Reference

```bash
# Load image and create a link to its source
"screenshot.png" image-load
dup image-info "source" get
"View original" swap link
```

### Copy Command Output to Clipboard

```bash
# Copy the current directory path
pwd clip-copy

# Copy a file's contents
"config.json" cat clip-copy
```

### Base64 Encode for Embedding

```bash
# Encode an image for HTML embedding
"icon.png" image-load to-base64
"data:image/png;base64," swap suffix
# => "data:image/png;base64,iVBORw0KGgo..."
```

## Troubleshooting

**Images display as text placeholders:**
- Your terminal may not support inline images
- Try iTerm2 or Kitty for full support
- Check that the file exists and is a valid image

**Clipboard operations fail:**
- Enable clipboard access in terminal preferences
- In tmux, set `set-clipboard on`
- SSH sessions may not support clipboard passthrough

**Link not clickable:**
- Terminal may not support OSC 8
- Try Cmd+click (macOS) or Ctrl+click (Linux)
- Some terminals require configuration

**image-load fails:**
- Check file path (use absolute paths or tilde expansion)
- Verify file permissions
- Ensure file is a supported image format
