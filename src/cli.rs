use hsab::Evaluator;
use crate::rcfile::{load_hsabrc, load_hsab_profile, load_stdlib, dirs_home, STDLIB_CONTENT};
use crate::terminal::execute_line;
use std::fs;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parsed command-line arguments
pub(crate) struct CliArgs {
    pub(crate) login: bool,
    pub(crate) command: Option<String>,
    pub(crate) script: Option<String>,
    pub(crate) help: bool,
    pub(crate) version: bool,
    pub(crate) init: bool,
    pub(crate) trace: bool,
}

/// Parse command-line arguments
pub(crate) fn parse_args(args: &[String]) -> CliArgs {
    let mut cli = CliArgs {
        login: false,
        command: None,
        script: None,
        help: false,
        version: false,
        init: false,
        trace: false,
    };

    let mut i = 1; // Skip program name
    while i < args.len() {
        match args[i].as_str() {
            "init" => {
                cli.init = true;
            }
            "-l" | "--login" => {
                cli.login = true;
            }
            "--trace" => {
                cli.trace = true;
            }
            "-c" => {
                // Everything after -c is the command
                if i + 1 < args.len() {
                    cli.command = Some(args[i + 1..].join(" "));
                    break;
                }
            }
            "--help" | "-h" => {
                cli.help = true;
            }
            "--version" | "-V" => {
                cli.version = true;
            }
            path => {
                // Assume it's a script file if not a flag
                if !path.starts_with('-') {
                    cli.script = Some(path.to_string());
                }
            }
        }
        i += 1;
    }

    cli
}

pub(crate) fn print_help() {
    println!(
        r#"hsab-{}£ Hash Backwards - A standalone stack-based postfix shell

USAGE:
    hsab                    Start interactive REPL
    hsab init               Install stdlib to ~/.hsab/lib/
    hsab -l, --login        Start as login shell (sources profile)
    hsab -c <command>       Execute a single command
    hsab <script.hsab>      Execute a script file
    hsab --help             Show this help message
    hsab --version          Show version

STARTUP:
    ~/.hsabrc               Executed on REPL startup (if exists)
    ~/.hsab/lib/stdlib.hsabrc  Auto-loaded if present (run 'hsab init')
    ~/.hsab_profile         Executed on login shell startup (-l flag)
    HSAB_BANNER=1           Show startup banner (quiet by default)

CORE CONCEPT:
    Values push to stack, executables pop args and push output.
    dest src cp             Stack: [dest] -> [dest, src] -> cp pops both
                            Result: cp dest src

SYNTAX:
    literal                 Push to stack
    "quoted"                Push quoted string
    \"\"\"...\"\"\"                 Multiline string (triple quotes)
    $VAR                    Push variable (expanded natively)
    ~/path                  Tilde expansion to home directory
    *.txt                   Glob expansion
    [expr ...]              Push block (deferred execution)
    :name                   Define: [block] :name stores block as word
    @                       Apply: execute top block
    |                       Pipe: producer [consumer] |
    > >> <                  Redirect stdout: [cmd] [file] >
    2> 2>>                  Redirect stderr: [cmd] [file] 2>
    &>                      Redirect both: [cmd] [file] &>
    && ||                   Logic: [left] [right] &&
    &                       Background: [cmd] &

STACK OPS:
    dup                     Duplicate top: a b -> a b b
    swap                    Swap top two: a b -> b a
    drop                    Remove top: a b -> a
    over                    Copy second: a b -> a b a
    rot                     Rotate three: a b c -> b c a
    depth                   Push stack size: a b c depth -> a b c 3

SNAPSHOTS:
    "name" snapshot         Save stack state with name
    snapshot                Save with auto-name, returns name
    "name" snapshot-restore Restore saved state
    snapshot-list           List all snapshots -> [names]
    "name" snapshot-delete  Delete a snapshot
    snapshot-clear          Clear all snapshots

PATH OPS:
    path-join               Join path: /dir file.txt path-join -> /dir/file.txt
    basename                Get name: /path/file.txt -> file
    dirname                 Get dir: /path/file.txt -> /path
    suffix                  Add suffix: file _bak -> file_bak
    reext                   Replace ext: file.txt .md -> file.md
    path-resolve            Resolve path: .. path-resolve -> /parent/of/cwd

STRING OPS:
    split1                  Split once: "a.b.c" "." split1 -> "a" "b.c"
    rsplit1                 Split once from right: "a.b.c" "." rsplit1 -> "a.b" "c"
    len                     String length: "hello" len -> 5
    slice                   Substring: "hello" 1 3 slice -> "ell"
    indexof                 Find index: "hello" "l" indexof -> 2
    str-replace             Replace: "hello" "l" "L" str-replace -> "heLLo"

PREDICATES:
    file? dir? exists?      File tests: path file? (exit 0 if file)
    empty?                  Check empty: val empty? (exit 0 if empty)
    eq? ne?                 Equality: a b eq? (exit 0 if equal)
    =? !=?                  Alias for eq?/ne?
    lt? gt? le? ge?         Numeric comparison: 3 5 lt? (exit 0 if 3 < 5)

ARITHMETIC:
    plus minus mul div mod  Math ops: 3 5 plus -> 8, 10 3 mod -> 1

LIST OPS:
    spread                  Split value by lines onto stack (with marker)
    each                    Apply block to each item: spread [block] each
    keep                    Filter: keep items where predicate passes
    collect                 Gather items back into single value
    map                     Transform: spread [block] map (each + collect)
    filter                  Filter: spread [pred] filter (keep + collect)

CONTROL FLOW:
    if                      Conditional: [cond] [then] [else] if
    times                   Repeat: 5 [hello echo] times
    while                   Loop: [cond] [body] while
    until                   Loop: [cond] [body] until
    break                   Exit current loop early

PARALLEL:
    parallel                [[cmd1] [cmd2]] parallel - run in parallel
    fork                    [cmd1] [cmd2] 2 fork - background N blocks

PROCESS SUBST:
    subst                   [cmd] subst - create temp file with output
    fifo                    [cmd] fifo - create named pipe with output

JSON / STRUCTURED DATA:
    json                    Parse JSON string to structured data
    unjson                  Convert structured data to JSON string
    ls-table                List directory as table: ls-table or /path ls-table

STRUCTURED DATA OPS:
    Record Operations:
      record                Create: "name" "Alice" "age" 30 record
      get                   Get field: record "name" get (supports "a.b.c" paths)
      set                   Set field: record "a.b" "val" set (deep set)
      del                   Delete field: record "name" del
      has?                  Check field: record "name" has? (exit 0/1)
      keys                  Get all keys: record keys
      values                Get all values: record values
      merge                 Combine records: rec1 rec2 merge

    Table Operations:
      table                 Create from records: marker rec1 rec2 table
      where                 Filter: table [predicate] where
      reject-where          Inverse of where: keep rows that DON'T match
      sort-by               Sort: table "column" sort-by
      select                Columns: table "col1" "col2" select
      first/last/nth        Row access: table 5 first
      group-by              Group: table "column" group-by
      unique/reverse        List transforms
      duplicates            Return items appearing more than once
      flatten               Flatten nested: list flatten
      reject                Inverse of keep: remove items that match predicate

    Error Handling:
      try                   Catch errors: [cmd] try
      error?                Check if error value (exit 0/1)
      throw                 Raise error: "message" throw

    Serialization (text -> structured):
      into-csv              "csv text" into-csv -> table
      into-tsv              "tsv text" into-tsv -> table
      into-json             "json text" into-json -> value
      into-lines            "text" into-lines -> list
      into-kv               "key=val" into-kv -> record
      into-delimited        "text" ";" into-delimited -> table (custom delimiter)

    Serialization (structured -> text):
      to-csv/to-json        table to-csv, table to-json
      to-tsv                table to-tsv -> TSV text
      to-delimited          table ";" to-delimited -> custom delimiter
      to-lines              list to-lines -> newline-separated text
      to-kv                 record to-kv -> key=value format

    File I/O:
      open                  "file.json" open -> auto-parse by extension
      save                  data "file.json" save -> auto-format by extension

    Auto-serialization: Tables/lists/records auto-convert when piped to external commands

    Aggregations:
      sum avg min max       "[1,2,3]" json sum -> 6
      count                 "[a,b,c]" json count -> 3
      reduce                list init [block] reduce -> fold over list

    Vector Operations (for embeddings):
      dot-product           vec1 vec2 dot-product -> scalar
      magnitude             vec magnitude -> L2 norm
      normalize             vec normalize -> unit vector
      cosine-similarity     vec1 vec2 cosine-similarity -> -1 to 1
      euclidean-distance    vec1 vec2 euclidean-distance -> scalar

    Type Introspection:
      typeof                42 typeof -> "Number"
      tap                   Inspect: val [echo] tap -> val (unchanged)
      dip                   Apply under: a b [+] dip -> (a+b) (original b)

    String Interpolation:
      format                name "Hello, {{}}!" format -> "Hello, Alice!"
                            bob alice "{{1}} meets {{0}}" format -> "alice meets bob"

    Combinators:
      fanout                val [op1] [op2] fanout -> result1 result2
                            Run value through multiple blocks, collect all results
      zip                   list1 list2 zip -> [[a1,b1], [a2,b2], ...]
                            Pair elements from two lists
      cross                 list1 list2 cross -> [[a1,b1], [a1,b2], ...]
                            Cartesian product of two lists
      retry                 N [block] retry -> result or error
                            Retry block up to N times until success
      compose               [op1] [op2] [op3] compose -> [op1 op2 op3]
                            Combine blocks into a single pipeline

RESOURCE LIMITS:
    timeout                 N [cmd] timeout - kill after N seconds

MODULE SYSTEM:
    .import                 Import module: "path.hsab" .import
                           With alias: "path.hsab" utils .import
    namespace::func         Call namespaced function
    _name                   Private definition (not exported)
    Search path: . -> ./lib/ -> ~/.hsab/lib/ -> $HSAB_PATH

PLUGINS (WASM):
    .plugin-load            Load plugin: "path/plugin.wasm" .plugin-load
    .plugin-unload          Unload: "plugin-name" .plugin-unload
    .plugin-reload          Force reload: "plugin-name" .plugin-reload
    .plugins                List all loaded plugins
    .plugin-info            Show plugin details: "name" .plugin-info
    Plugin Directory: ~/.hsab/plugins/
    Manifest Format: plugin.toml (TOML config)
    Hot Reload: Enabled (watches for .wasm changes)

META COMMANDS (dot-prefixed, affect shell state):
    .export                 Set environment variable: VAR=val .export
    .unset                  Remove environment variable: VAR .unset
    .env                    List all environment variables
    .jobs                   List background jobs
    .fg                     Bring job to foreground: %1 .fg
    .bg                     Resume job in background: %1 .bg
    .exit                   Exit the shell: .exit, 0 .exit
    .tty                    Run interactive command: file.txt vim .tty
    .source / .             Execute file in current context: file.hsab .source
    .hash                   Show/manage command hash table: ls .hash, -r .hash
    .type                   Show how a word resolves: ls .type
    .which                  Find executable path: ls .which
    .alias                  Define alias: "ll" "-la ls" .alias
    .unalias                Remove alias: ll .unalias
    .trap                   Set signal handler: [cleanup] SIGINT .trap
    .copy                   Copy top to clipboard: value .copy
    .cut                    Cut top to clipboard (drop + copy): value .cut
    .paste                  Paste from clipboard onto stack

SHELL BUILTINS (both .dot and non-dot forms for POSIX compat):
    .cd / cd                Change directory (with ~ expansion)
    .pwd / pwd              Print working directory
    .echo / echo            Echo arguments (no fork)
    .printf / printf        Formatted print: "Hello %s" name .printf
    .test / test / [        File and string tests (postfix: file.txt -f .test)
    .true / true            Exit with 0
    .false / false          Exit with 1
    .read / read            Read line into variable: varname .read
    .wait / wait            Wait for background jobs: .wait, %1 .wait
    .kill / kill            Send signal to job: %1 .kill, %1 -9 .kill
    .pushd / pushd          Push directory: /tmp .pushd
    .popd / popd            Pop directory: .popd
    .dirs / dirs            Show directory stack: .dirs
    .local / local          Declare local variable: VAR .local
    .return / return        Return from function: .return, 0 .return

COMMENTS:
    # comment               Inline comments (ignored)

REPL COMMANDS:
    .help, .h               Show this help
    .stack, .s              Show current stack
    .peek, .k               Show top value without popping
    .pop, .p                Pop and show top value
    clear, .clear, .c       Clear stack and screen
    clear-stack             Clear the stack only
    clear-screen            Clear the screen only
    .use, .u                Move top stack item to input
    .use=N, .u=N            Move N stack items to input
    .types, .t              Toggle type annotations in hint
    .hint                   Toggle hint visibility
    exit, quit              Exit the REPL

DEBUGGER:
    .debug, .d              Toggle debug mode (step through expressions)
    .break <pat>, .b <pat>  Set breakpoint on expression pattern
    .delbreak <pat>, .db    Remove a breakpoint
    .breakpoints, .bl       List all breakpoints
    .clearbreaks, .cb       Clear all breakpoints
    .step                   Enable single-step mode
    When paused:
      n/next/Enter          Step to next expression
      c/continue            Continue until next breakpoint
      s/stack               Show full stack
      b/breakpoints         List breakpoints
      q/quit                Quit debug mode

KEYBOARD SHORTCUTS:
    Alt+↑                   Push first word from input to stack
    Alt+↓                   Pop one from stack to input
    Alt+A                   Push ALL words from input to stack
    Alt+a                   Pop ALL from stack to input
    Alt+k                   Clear/discard the entire stack
    Alt+c                   Copy top to system clipboard
    Alt+x                   Cut top to system clipboard (drop + copy)
    Alt+t                   Toggle type annotations in hint
    Alt+h                   Toggle hint visibility
    (Ctrl+O also pops one, for terminal compatibility)

CLIPBOARD:
    .copy                   Copy top to clipboard (non-destructive)
    .cut                    Cut top to clipboard (pop + copy)
    .paste                  Paste from clipboard onto stack
    paste-here              Literal that expands to clipboard contents
                            (like $VAR but for clipboard)

EXAMPLES:
    hello echo                    # echo hello
    -la ls                        # ls -la
    world hello echo              # echo world hello (LIFO)
    pwd ls                        # ls $(pwd) (command substitution)
    [hello echo] @                # Apply block: echo hello
    ls [grep txt] |               # Pipe: ls | grep txt
    file.txt dup .bak reext cp    # cp file.txt file.bak
    [dup .bak reext cp] :backup   # Define 'backup' word
    file.txt backup               # Use it: cp file.txt file.bak
    ~/Documents ls                # Tilde expansion
    *.rs wc -l                    # Glob expansion
    /tmp cd pwd                   # Change directory
    5 [10 sleep] timeout          # Kill after 5 seconds
    '{{"name":"test"}}' json      # Parse JSON to structured data
    "utils.hsab" .import          # Import module as utils::
    file.txt utils::backup        # Call namespaced function
"#,
        VERSION
    );
}

pub(crate) fn print_version() {
    println!("hsab-{}£", VERSION);
}

/// Execute a single command with optional login shell mode
pub(crate) fn execute_command_with_login(cmd: &str, is_login: bool, trace: bool) -> ExitCode {
    let mut eval = Evaluator::new();
    eval.set_trace_mode(trace);

    // Load profile if login shell
    if is_login {
        load_hsab_profile(&mut eval);
    }

    // Load stdlib first (provides defaults)
    load_stdlib(&mut eval);

    // Load ~/.hsabrc (user customizations override stdlib)
    load_hsabrc(&mut eval);

    match execute_line(&mut eval, cmd, true) {
        Ok(exit_code) => {
            if exit_code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(exit_code as u8)
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Execute a script file
pub(crate) fn execute_script(path: &str, trace: bool) -> ExitCode {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let mut eval = Evaluator::new();
    eval.set_trace_mode(trace);

    // Load stdlib if installed
    load_stdlib(&mut eval);

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        match execute_line(&mut eval, trimmed, true) {
            Ok(exit_code) => {
                // Clear the stack after each line (like .hsabrc loading)
                // Output was already printed by execute_line
                eval.clear_stack();

                if exit_code != 0 {
                    eprintln!("Error at line {}: command failed with exit code {}",
                             line_num + 1, exit_code);
                    return ExitCode::FAILURE;
                }
            }
            Err(e) => {
                eprintln!("Error at line {}: {}", line_num + 1, e);
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

/// Initialize hsab stdlib: create ~/.hsab/lib/ and install stdlib.hsabrc
pub(crate) fn run_init() -> ExitCode {
    let home = match dirs_home() {
        Some(h) => h,
        None => {
            eprintln!("Error: Could not determine home directory");
            return ExitCode::FAILURE;
        }
    };

    let lib_dir = home.join(".hsab").join("lib");
    let stdlib_file = lib_dir.join("stdlib.hsabrc");

    // Create directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&lib_dir) {
        eprintln!("Error creating {}: {}", lib_dir.display(), e);
        return ExitCode::FAILURE;
    }

    // Check if stdlib already exists
    if stdlib_file.exists() {
        println!("Stdlib already installed at {}", stdlib_file.display());
        println!("To reinstall, remove the file first:");
        println!("  rm {}", stdlib_file.display());
        return ExitCode::SUCCESS;
    }

    // Write stdlib content
    if let Err(e) = fs::write(&stdlib_file, STDLIB_CONTENT) {
        eprintln!("Error writing {}: {}", stdlib_file.display(), e);
        return ExitCode::FAILURE;
    }

    println!("\u{2713} Installed stdlib to {}", stdlib_file.display());
    println!();
    println!("The stdlib is now auto-loaded on startup. It includes:");
    println!("  \u{2022} Arithmetic: abs, min, max, inc, dec");
    println!("  \u{2022} String predicates: contains?, starts?, ends?");
    println!("  \u{2022} Navigation: ll, la, l1, lt, lS");
    println!("  \u{2022} Path ops: dirname, basename, reext, backup");
    println!("  \u{2022} Git shortcuts: gs, gd, gl, ga, gcm");
    println!("  \u{2022} Stack helpers: nip, tuck, -rot, 2drop, 2dup");
    println!("  \u{2022} And more... see: {}", stdlib_file.display());

    ExitCode::SUCCESS
}
