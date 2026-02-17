use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::process::{Command, Stdio};

impl Evaluator {
    /// Execute a command, popping args from stack
    pub(crate) fn execute_command(&mut self, cmd: &str) -> Result<(), EvalError> {
        // Collect args from stack (LIFO - pop until we hit a block, marker, or empty)
        let mut args = Vec::new();
        while let Some(value) = self.stack.last() {
            match value {
                Value::Block(_) => break,
                Value::Marker => break,
                Value::Nil => {
                    self.stack.pop();
                    // Skip nil values
                }
                _ => {
                    if let Some(arg) = value.as_arg() {
                        // Expand globs and tilde for each argument
                        args.extend(self.expand_arg(&arg));
                    }
                    self.stack.pop();
                }
            }
        }

        // Try builtin first
        if let Some(result) = self.try_builtin(cmd, &args) {
            return result;
        }

        // Execute native command
        let (output, exit_code) = self.execute_native(cmd, args)?;
        self.last_exit_code = exit_code;

        if output.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Execute a native command using std::process::Command
    /// Uses capture_mode to decide whether to capture output or run interactively
    pub(crate) fn execute_native(&mut self, cmd: &str, args: Vec<String>) -> Result<(String, i32), EvalError> {
        // Only run interactively if:
        // 1. capture_mode is false (nothing will consume the output)
        // 2. stdout is a TTY (we're in an interactive context)
        let run_interactive = !self.capture_mode && Self::is_interactive();

        if run_interactive {
            // Run interactively - output goes directly to terminal
            let status = Command::new(cmd)
                .args(&args)
                .current_dir(&self.cwd)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd, e)))?;

            Ok((String::new(), status.code().unwrap_or(-1)))
        } else {
            // Capture output (for piping, scripts, tests, or when output is consumed)
            let output = Command::new(cmd)
                .args(&args)
                .current_dir(&self.cwd)
                .output()
                .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd, e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            Ok((stdout, exit_code))
        }
    }

    /// Check if we're running in an interactive context (TTY)
    pub(crate) fn is_interactive() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
    }

    /// Try to execute a builtin command
    pub(crate) fn try_builtin(&mut self, cmd: &str, args: &[String]) -> Option<Result<(), EvalError>> {
        match cmd {
            // Borderlines: both formats (POSIX compat + dot-convention)
            "cd" | ".cd" => Some(self.builtin_cd(args)),
            "pwd" | ".pwd" => Some(self.builtin_pwd()),
            "echo" | ".echo" => Some(self.builtin_echo(args)),
            "true" | ".true" => Some(self.builtin_true()),
            "false" | ".false" => Some(self.builtin_false()),
            "test" | "[" | ".test" => Some(self.builtin_test(args)),
            "read" | ".read" => Some(self.builtin_read(args)),
            "printf" | ".printf" => Some(self.builtin_printf(args)),
            "wait" | ".wait" => Some(self.builtin_wait(args)),
            "kill" | ".kill" => Some(self.builtin_kill(args)),
            "pushd" | ".pushd" => Some(self.builtin_pushd(args)),
            "popd" | ".popd" => Some(self.builtin_popd(args)),
            "dirs" | ".dirs" => Some(self.builtin_dirs(args)),
            // Note: local is now handled in try_structured_builtin to preserve Value types
            "return" | ".return" => Some(self.builtin_return(args)),
            // Meta commands: dot-only (shell state manipulation)
            ".export" => Some(self.builtin_export(args)),
            ".unset" => Some(self.builtin_unset(args)),
            ".env" => Some(self.builtin_env()),
            ".jobs" => Some(self.builtin_jobs()),
            ".fg" => Some(self.builtin_fg(args)),
            ".bg" => Some(self.builtin_bg(args)),
            ".exit" => Some(self.builtin_exit(args)),
            ".tty" => Some(self.builtin_tty(args)),
            ".which" => Some(self.builtin_which(args)),
            ".source" | "." => Some(self.builtin_source(args)),
            ".hash" => Some(self.builtin_hash(args)),
            ".type" => Some(self.builtin_type(args)),
            ".alias" => Some(self.builtin_alias(args)),
            ".unalias" => Some(self.builtin_unalias(args)),
            ".trap" => Some(self.builtin_trap(args)),
            // Stack-native predicates
            "file?" => Some(self.builtin_file_predicate(args)),
            "dir?" => Some(self.builtin_dir_predicate(args)),
            "exists?" => Some(self.builtin_exists_predicate(args)),
            "empty?" => Some(self.builtin_empty_predicate(args)),
            "contains?" => Some(self.builtin_contains_predicate(args)),
            "starts?" => Some(self.builtin_starts_predicate(args)),
            "ends?" => Some(self.builtin_ends_predicate(args)),
            // String primitives
            "len" => Some(self.builtin_len(args)),
            "slice" => Some(self.builtin_slice(args)),
            "indexof" => Some(self.builtin_indexof(args)),
            "str-replace" => Some(self.builtin_str_replace(args)),
            "format" => Some(self.builtin_format(args)),
            // Path operations (native implementation for performance)
            "reext" => Some(self.builtin_reext(args)),
            // Type predicates
            "number?" => Some(self.builtin_number_predicate()),
            "string?" => Some(self.builtin_string_predicate()),
            "array?" => Some(self.builtin_array_predicate()),
            "function?" => Some(self.builtin_function_predicate()),
            // Logical operators
            "not" => Some(self.builtin_not()),
            "xor" => Some(self.builtin_xor()),
            "nand" => Some(self.builtin_nand()),
            "nor" => Some(self.builtin_nor()),
            // Phase 0: Type introspection
            "typeof" => Some(self.builtin_typeof()),
            // Phase 1: Record operations
            "record" => Some(self.builtin_record()),
            "get" => Some(self.builtin_get()),
            "set" => Some(self.builtin_set()),
            "del" => Some(self.builtin_del()),
            "has?" => Some(self.builtin_has()),
            "keys" => Some(self.builtin_keys()),
            "values" => Some(self.builtin_values()),
            "merge" => Some(self.builtin_merge()),
            // Phase 2: Table operations
            "table" => Some(self.builtin_table()),
            "where" => Some(self.builtin_where()),
            "sort-by" => Some(self.builtin_sort_by()),
            "select" => Some(self.builtin_select()),
            "first" => Some(self.builtin_first()),
            "last" => Some(self.builtin_last()),
            "nth" => Some(self.builtin_nth()),
            // Phase 3: Error handling
            "try" => Some(self.builtin_try()),
            "error?" => Some(self.builtin_error_predicate()),
            "throw" => Some(self.builtin_throw()),
            // Phase 4: Serialization bridge
            // into-X = serialize (structured -> text), from-X = parse (text -> structured)
            "into-json" | "to-json" => Some(self.builtin_to_json()),
            "from-json" => Some(self.builtin_into_json()),
            "into-csv" | "to-csv" => Some(self.builtin_to_csv()),
            "from-csv" => Some(self.builtin_into_csv()),
            "into-lines" | "from-lines" => Some(self.builtin_into_lines()),
            "to-lines" => Some(self.builtin_to_lines()),
            "into-kv" | "to-kv" => Some(self.builtin_to_kv()),
            "from-kv" => Some(self.builtin_into_kv()),
            "into-tsv" | "to-tsv" => Some(self.builtin_to_tsv()),
            "from-tsv" => Some(self.builtin_into_tsv()),
            "to-delimited" => Some(self.builtin_to_delimited()),
            // File operations
            "save" => Some(self.builtin_save()),
            // Additional aggregations
            "reduce" => Some(self.builtin_reduce()),
            "fold" => Some(self.builtin_fold()),
            "bend" => Some(self.builtin_bend()),
            // Additional list/table operations
            "reject" => Some(self.builtin_reject()),
            "reject-where" => Some(self.builtin_reject_where()),
            "duplicates" => Some(self.builtin_duplicates()),
            // Vector operations (for embeddings)
            "dot-product" => Some(self.builtin_dot_product()),
            "magnitude" => Some(self.builtin_magnitude()),
            "normalize" => Some(self.builtin_normalize()),
            "cosine-similarity" => Some(self.builtin_cosine_similarity()),
            "euclidean-distance" => Some(self.builtin_euclidean_distance()),
            // Phase 10: Combinators
            "fanout" => Some(self.builtin_fanout()),
            "zip" => Some(self.builtin_zip()),
            "cross" => Some(self.builtin_cross()),
            "retry" => Some(self.builtin_retry()),
            "retry-delay" => Some(self.builtin_retry_delay()),
            "compose" => Some(self.builtin_compose()),
            // Plugin management builtins (meta commands)
            #[cfg(feature = "plugins")]
            ".plugin-load" => Some(self.builtin_plugin_load(args)),
            #[cfg(feature = "plugins")]
            ".plugin-unload" => Some(self.builtin_plugin_unload(args)),
            #[cfg(feature = "plugins")]
            ".plugin-reload" => Some(self.builtin_plugin_reload(args)),
            #[cfg(feature = "plugins")]
            ".plugins" => Some(self.builtin_plugin_list()),
            #[cfg(feature = "plugins")]
            ".plugin-info" => Some(self.builtin_plugin_info(args)),
            // Stack snapshots
            "snapshot" => Some(self.builtin_snapshot(args)),
            "snapshot-restore" => Some(self.builtin_snapshot_restore(args)),
            "snapshot-list" => Some(self.builtin_snapshot_list()),
            "snapshot-delete" => Some(self.builtin_snapshot_delete(args)),
            "snapshot-clear" => Some(self.builtin_snapshot_clear()),
            // Async delays (take args)
            "delay" => Some(self.builtin_delay(args)),
            "delay-async" => Some(self.builtin_delay_async(args)),
            _ => None,
        }
    }

    /// Try to handle structured data builtins directly (without stringifying args)
    /// Returns true if handled, false if should fall through to execute_command
    pub(crate) fn try_structured_builtin(&mut self, cmd: &str) -> Result<bool, EvalError> {
        match cmd {
            // Local variable (stack-native to preserve structured values)
            "local" | ".local" => { self.builtin_local_stack()?; Ok(true) }
            // Phase 0
            "typeof" => { self.builtin_typeof()?; Ok(true) }
            // Phase 1: Record ops
            "record" => { self.builtin_record()?; Ok(true) }
            "get" => { self.builtin_get()?; Ok(true) }
            "set" => { self.builtin_set()?; Ok(true) }
            "del" => { self.builtin_del()?; Ok(true) }
            "has?" => { self.builtin_has()?; Ok(true) }
            "keys" => { self.builtin_keys()?; Ok(true) }
            "values" => { self.builtin_values()?; Ok(true) }
            "merge" => { self.builtin_merge()?; Ok(true) }
            // Phase 2: Table ops
            "table" => { self.builtin_table()?; Ok(true) }
            "where" => { self.builtin_where()?; Ok(true) }
            "sort-by" => { self.builtin_sort_by()?; Ok(true) }
            "select" => { self.builtin_select()?; Ok(true) }
            "first" => { self.builtin_first()?; Ok(true) }
            "last" => { self.builtin_last()?; Ok(true) }
            "nth" => { self.builtin_nth()?; Ok(true) }
            // Type predicates
            "number?" => { self.builtin_number_predicate()?; Ok(true) }
            "string?" => { self.builtin_string_predicate()?; Ok(true) }
            "array?" => { self.builtin_array_predicate()?; Ok(true) }
            "function?" => { self.builtin_function_predicate()?; Ok(true) }
            // Logical operators
            "not" => { self.builtin_not()?; Ok(true) }
            "xor" => { self.builtin_xor()?; Ok(true) }
            "nand" => { self.builtin_nand()?; Ok(true) }
            "nor" => { self.builtin_nor()?; Ok(true) }
            // Phase 3: Error handling
            "try" => { self.builtin_try()?; Ok(true) }
            "error?" => { self.builtin_error_predicate()?; Ok(true) }
            "nil?" => { self.builtin_nil_predicate()?; Ok(true) }
            "throw" => { self.builtin_throw()?; Ok(true) }
            // Phase 4: Serialization
            // into-X = serialize (structured -> text), from-X = parse (text -> structured)
            "into-json" | "to-json" => { self.builtin_to_json()?; Ok(true) }
            "from-json" => { self.builtin_into_json()?; Ok(true) }
            "into-csv" | "to-csv" => { self.builtin_to_csv()?; Ok(true) }
            "from-csv" => { self.builtin_into_csv()?; Ok(true) }
            "into-lines" | "from-lines" => { self.builtin_into_lines()?; Ok(true) }
            "to-lines" => { self.builtin_to_lines()?; Ok(true) }
            "into-kv" | "to-kv" => { self.builtin_to_kv()?; Ok(true) }
            "from-kv" => { self.builtin_into_kv()?; Ok(true) }
            "into-tsv" | "to-tsv" => { self.builtin_to_tsv()?; Ok(true) }
            "from-tsv" => { self.builtin_into_tsv()?; Ok(true) }
            "to-delimited" => { self.builtin_to_delimited()?; Ok(true) }
            // Phase 5: Stack utilities
            "tap" => { self.builtin_tap()?; Ok(true) }
            "dip" => { self.builtin_dip()?; Ok(true) }
            "dig" | "pick" => { self.stack_dig()?; Ok(true) }
            "bury" | "roll" => { self.stack_bury()?; Ok(true) }
            // Phase 6: Aggregations
            "sum" => { self.builtin_sum()?; Ok(true) }
            "avg" => { self.builtin_avg()?; Ok(true) }
            "min" => { self.builtin_min()?; Ok(true) }
            "max" => { self.builtin_max()?; Ok(true) }
            "count" => { self.builtin_count()?; Ok(true) }
            "reduce" => { self.builtin_reduce()?; Ok(true) }
            "fold" => { self.builtin_fold()?; Ok(true) }
            "bend" => { self.builtin_bend()?; Ok(true) }
            // Phase 6.5: Statistical functions
            "product" => { self.builtin_product()?; Ok(true) }
            "median" => { self.builtin_median()?; Ok(true) }
            "mode" => { self.builtin_mode()?; Ok(true) }
            "modes" => { self.builtin_modes()?; Ok(true) }
            "variance" => { self.builtin_variance()?; Ok(true) }
            "sample-variance" => { self.builtin_sample_variance()?; Ok(true) }
            "stdev" => { self.builtin_stdev()?; Ok(true) }
            "sample-stdev" => { self.builtin_sample_stdev()?; Ok(true) }
            "percentile" => { self.builtin_percentile()?; Ok(true) }
            "five-num" => { self.builtin_five_num()?; Ok(true) }
            // Phase 8: Extended table ops
            "group-by" => { self.builtin_group_by()?; Ok(true) }
            "unique" => { self.builtin_unique()?; Ok(true) }
            "reverse" => { self.builtin_reverse()?; Ok(true) }
            "flatten" => { self.builtin_flatten()?; Ok(true) }
            "reject" => { self.builtin_reject()?; Ok(true) }
            "reject-where" => { self.builtin_reject_where()?; Ok(true) }
            "duplicates" => { self.builtin_duplicates()?; Ok(true) }
            // Extended spread operations
            "fields" => { self.builtin_fields()?; Ok(true) }
            "fields-keys" => { self.builtin_fields_keys()?; Ok(true) }
            "spread-head" => { self.builtin_spread_head()?; Ok(true) }
            "spread-tail" => { self.builtin_spread_tail()?; Ok(true) }
            "spread-n" => { self.builtin_spread_n()?; Ok(true) }
            "spread-to" => { self.builtin_spread_to()?; Ok(true) }
            // Phase 9: Vector operations
            "dot-product" => { self.builtin_dot_product()?; Ok(true) }
            "magnitude" => { self.builtin_magnitude()?; Ok(true) }
            "normalize" => { self.builtin_normalize()?; Ok(true) }
            "cosine-similarity" => { self.builtin_cosine_similarity()?; Ok(true) }
            "euclidean-distance" => { self.builtin_euclidean_distance()?; Ok(true) }
            // Phase 10: Combinators
            "fanout" => { self.builtin_fanout()?; Ok(true) }
            "zip" => { self.builtin_zip()?; Ok(true) }
            "cross" => { self.builtin_cross()?; Ok(true) }
            "retry" => { self.builtin_retry()?; Ok(true) }
            "compose" => { self.builtin_compose()?; Ok(true) }
            // Phase 11: Additional parsers (from-X aliases for parsing)
            "from-delimited" | "into-delimited" => { self.builtin_into_delimited()?; Ok(true) }
            // Structured builtins
            "ls-table" => { self.builtin_ls_table()?; Ok(true) }
            "open" => { self.builtin_open()?; Ok(true) }
            "save" => { self.builtin_save()?; Ok(true) }
            // Media / Image operations
            "image-load" => { self.builtin_image_load_stack()?; Ok(true) }
            "image-show" => { self.builtin_image_show()?; Ok(true) }
            "image-info" => { self.builtin_image_info()?; Ok(true) }
            "to-base64" => { self.builtin_to_base64()?; Ok(true) }
            "from-base64" => { self.builtin_from_base64()?; Ok(true) }
            // Link operations (OSC 8)
            "link" => { self.builtin_link()?; Ok(true) }
            "link-info" => { self.builtin_link_info()?; Ok(true) }
            // Clipboard operations (OSC 52)
            ".copy" => { self.builtin_clip_copy()?; Ok(true) }
            ".cut" => { self.builtin_clip_cut()?; Ok(true) }
            ".paste" => { self.builtin_clip_paste()?; Ok(true) }
            // Encoding operations
            "to-hex" => { self.builtin_to_hex()?; Ok(true) }
            "from-hex" => { self.builtin_from_hex()?; Ok(true) }
            "as-bytes" => { self.builtin_as_bytes()?; Ok(true) }
            "to-bytes" => { self.builtin_to_bytes_list()?; Ok(true) }
            "to-string" => { self.builtin_bytes_to_string()?; Ok(true) }
            "read-bytes" => { self.builtin_read_bytes()?; Ok(true) }
            // Hash functions (SHA-2)
            "sha256" => { self.builtin_sha256()?; Ok(true) }
            "sha384" => { self.builtin_sha384()?; Ok(true) }
            "sha512" => { self.builtin_sha512()?; Ok(true) }
            // Hash functions (SHA-3)
            "sha3-256" => { self.builtin_sha3_256()?; Ok(true) }
            "sha3-384" => { self.builtin_sha3_384()?; Ok(true) }
            "sha3-512" => { self.builtin_sha3_512()?; Ok(true) }
            // File hash functions
            "sha256-file" => { self.builtin_sha256_file()?; Ok(true) }
            "sha3-256-file" => { self.builtin_sha3_256_file()?; Ok(true) }
            // Bytes len (try first, fallback to string len)
            "len" => {
                // Try Bytes len first
                if let Some(Value::Bytes(_)) = self.stack.last() {
                    self.builtin_bytes_len()?;
                    Ok(true)
                } else {
                    Ok(false) // Fall through to string len
                }
            }
            // BigInt operations
            "to-bigint" => { self.builtin_to_bigint()?; Ok(true) }
            "big-add" => { self.builtin_big_add()?; Ok(true) }
            "big-sub" => { self.builtin_big_sub()?; Ok(true) }
            "big-mul" => { self.builtin_big_mul()?; Ok(true) }
            "big-div" => { self.builtin_big_div()?; Ok(true) }
            "big-mod" => { self.builtin_big_mod()?; Ok(true) }
            "big-xor" => { self.builtin_big_xor()?; Ok(true) }
            "big-and" => { self.builtin_big_and()?; Ok(true) }
            "big-or" => { self.builtin_big_or()?; Ok(true) }
            "big-eq?" => { self.builtin_big_eq()?; Ok(true) }
            "big-lt?" => { self.builtin_big_lt()?; Ok(true) }
            "big-gt?" => { self.builtin_big_gt()?; Ok(true) }
            "big-shl" => { self.builtin_big_shl()?; Ok(true) }
            "big-shr" => { self.builtin_big_shr()?; Ok(true) }
            "big-pow" => { self.builtin_big_pow()?; Ok(true) }
            // Predicates (stack-native to avoid greedy arg collection)
            "eq?" => { self.builtin_eq_stack()?; Ok(true) }
            "ne?" => { self.builtin_ne_stack()?; Ok(true) }
            "=?" => { self.builtin_num_eq_stack()?; Ok(true) }
            "!=?" => { self.builtin_num_ne_stack()?; Ok(true) }
            "lt?" => { self.builtin_lt_stack()?; Ok(true) }
            "gt?" => { self.builtin_gt_stack()?; Ok(true) }
            "le?" => { self.builtin_le_stack()?; Ok(true) }
            "ge?" => { self.builtin_ge_stack()?; Ok(true) }
            // Arithmetic primitives (stack-native to avoid greedy arg collection)
            "plus" => { self.builtin_plus_stack()?; Ok(true) }
            "minus" => { self.builtin_minus_stack()?; Ok(true) }
            "mul" => { self.builtin_mul_stack()?; Ok(true) }
            "div" => { self.builtin_div_stack()?; Ok(true) }
            "mod" => { self.builtin_mod_stack()?; Ok(true) }
            // Math primitives (for stats support)
            "pow" => { self.builtin_pow()?; Ok(true) }
            "sqrt" => { self.builtin_sqrt()?; Ok(true) }
            "floor" => { self.builtin_floor()?; Ok(true) }
            "ceil" => { self.builtin_ceil()?; Ok(true) }
            "round" => { self.builtin_round()?; Ok(true) }
            "idiv" => { self.builtin_idiv()?; Ok(true) }
            "sort-nums" => { self.builtin_sort_nums()?; Ok(true) }
            "log-base" => { self.builtin_log_base()?; Ok(true) }
            // Async / concurrent operations
            "async" => { self.builtin_async()?; Ok(true) }
            "await" => { self.builtin_await()?; Ok(true) }
            "future-status" => { self.builtin_future_status()?; Ok(true) }
            "future-result" => { self.builtin_future_result()?; Ok(true) }
            "future-cancel" => { self.builtin_future_cancel()?; Ok(true) }
            "parallel-n" => { self.builtin_parallel_n()?; Ok(true) }
            "parallel-map" => { self.builtin_parallel_map()?; Ok(true) }
            "race" => { self.builtin_race()?; Ok(true) }
            "await-all" => { self.builtin_await_all()?; Ok(true) }
            "future-race" => { self.builtin_future_race()?; Ok(true) }
            "future-await-n" => { self.builtin_future_await_n()?; Ok(true) }
            "futures-list" => { self.builtin_futures_list()?; Ok(true) }
            "future-map" => { self.builtin_future_map()?; Ok(true) }
            "retry-delay" => { self.builtin_retry_delay()?; Ok(true) }
            // HTTP client operations
            "fetch" => { self.builtin_fetch()?; Ok(true) }
            "fetch-status" => { self.builtin_fetch_status()?; Ok(true) }
            "fetch-headers" => { self.builtin_fetch_headers()?; Ok(true) }
            // Macro-generated builtins (proof of concept)
            "abs" => { self.builtin_abs()?; Ok(true) }
            "negate" => { self.builtin_negate()?; Ok(true) }
            "max-of" => { self.builtin_max_of()?; Ok(true) }
            "min-of" => { self.builtin_min_of()?; Ok(true) }
            // Unicode operator aliases
            "Σ" => { self.builtin_sum()?; Ok(true) }
            "Π" => { self.builtin_product()?; Ok(true) }
            "÷" => { self.builtin_div_stack()?; Ok(true) }
            "⋅" => { self.builtin_mul_stack()?; Ok(true) }
            "√" => { self.builtin_sqrt()?; Ok(true) }
            "∅" => { self.stack.push(Value::Nil); self.last_exit_code = 0; Ok(true) }
            "≠" => { self.builtin_ne_stack()?; Ok(true) }
            "≤" => { self.builtin_le_stack()?; Ok(true) }
            "≥" => { self.builtin_ge_stack()?; Ok(true) }
            "μ" => { self.builtin_avg()?; Ok(true) }
            // Watch mode
            #[cfg(feature = "plugins")]
            "watch" => { self.builtin_watch()?; Ok(true) }
            // Stack-native shell operations (override existing where applicable)
            "cd" | ".cd" => { self.builtin_cd_native()?; Ok(true) }
            "touch" => { self.builtin_touch()?; Ok(true) }
            "mkdir" => { self.builtin_mkdir_native()?; Ok(true) }
            "mkdir-p" => { self.builtin_mkdir_p()?; Ok(true) }
            "mktemp" => { self.builtin_mktemp()?; Ok(true) }
            "mktemp-d" => { self.builtin_mktemp_d()?; Ok(true) }
            "cp" => { self.builtin_cp()?; Ok(true) }
            "mv" => { self.builtin_mv()?; Ok(true) }
            "rm" => { self.builtin_rm()?; Ok(true) }
            "rm-r" => { self.builtin_rm_r()?; Ok(true) }
            "ln" => { self.builtin_ln()?; Ok(true) }
            "realpath" => { self.builtin_realpath()?; Ok(true) }
            "which" => { self.builtin_which_native()?; Ok(true) }
            // Note: dirname/basename handled by parser as Expr::Dirname/Basename
            "extname" => { self.builtin_extname()?; Ok(true) }
            "glob" => { self.builtin_glob()?; Ok(true) }
            "ls" => { self.builtin_ls_native()?; Ok(true) }
            _ => Ok(false),
        }
    }

    /// Try to execute a plugin command (returns true if handled)
    #[allow(unused_variables)]
    pub(crate) fn try_plugin_command_if_enabled(&mut self, cmd: &str) -> Result<bool, EvalError> {
        #[cfg(feature = "plugins")]
        {
            // First, check if this command is provided by a plugin
            let has_cmd = self.plugin_host.as_ref().map(|h| h.has_command(cmd)).unwrap_or(false);

            if !has_cmd {
                // Check for hot reloads even if this isn't a plugin command
                if let Some(ref mut host) = self.plugin_host {
                    if let Ok(reloaded) = host.check_hot_reload() {
                        for name in &reloaded {
                            eprintln!("Plugin reloaded: {}", name);
                        }
                    }
                }
                return Ok(false);
            }

            // Sync stack to shared stack before calling plugin
            self.sync_stack_to_plugins();

            // Collect args from stack (for passing as JSON)
            let mut args = Vec::new();
            while let Some(value) = self.stack.last() {
                match value {
                    Value::Block(_) | Value::Marker | Value::Nil => break,
                    _ => {
                        if let Some(arg) = value.as_arg() {
                            args.push(arg);
                        }
                        self.stack.pop();
                    }
                }
            }
            args.reverse(); // LIFO -> correct order

            // Call the plugin command
            if let Some(ref mut host) = self.plugin_host {
                match host.call(cmd, &args) {
                    Ok(exit_code) => {
                        // Sync stack back from plugins
                        self.sync_stack_from_plugins();
                        self.last_exit_code = exit_code;
                        return Ok(true);
                    }
                    Err(e) => {
                        return Err(EvalError::ExecError(format!("Plugin error: {}", e)));
                    }
                }
            }
        }

        Ok(false)
    }

    /// Sync the evaluator's stack to the shared plugin stack
    #[cfg(feature = "plugins")]
    pub(crate) fn sync_stack_to_plugins(&self) {
        if let Ok(mut shared) = self.shared_stack.lock() {
            shared.clear();
            shared.extend(self.stack.clone());
        }
    }

    /// Sync the shared plugin stack back to the evaluator
    #[cfg(feature = "plugins")]
    pub(crate) fn sync_stack_from_plugins(&mut self) {
        if let Ok(shared) = self.shared_stack.lock() {
            self.stack = shared.clone();
        }
    }
}
