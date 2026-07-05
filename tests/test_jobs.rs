//! Integration tests for background jobs and SIGCHLD reaping (issue #30)

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, lex, parse, Evaluator};

use std::time::Duration;

/// Run a line on a persistent evaluator and return the output.
fn run(evaluator: &mut Evaluator, input: &str) -> String {
    let tokens = lex(input).unwrap();
    let program = parse(tokens).unwrap();
    let result = evaluator.eval(&program).unwrap();
    evaluator.clear_stack();
    result.output
}

/// Extract the pid (second tab-separated field) from a `.jobs` output line.
fn first_job_pid(jobs_output: &str) -> Option<u32> {
    jobs_output
        .lines()
        .next()?
        .split('\t')
        .nth(1)?
        .trim()
        .parse()
        .ok()
}

#[test]
fn test_background_job_transitions_to_done() {
    let mut evaluator = Evaluator::new();
    run(&mut evaluator, "#[0.1 sleep] &");

    // Right after spawn, the job should be listed as Running
    let out = run(&mut evaluator, ".jobs");
    assert!(
        out.contains("Running"),
        "job should be Running right after spawn: {}",
        out
    );

    // After the child exits, .jobs must reflect Done without manual help
    std::thread::sleep(Duration::from_millis(400));
    let out = run(&mut evaluator, ".jobs");
    assert!(
        out.contains("Done"),
        "job should transition to Done after exit: {}",
        out
    );
}

#[test]
#[cfg(unix)]
fn test_background_job_is_reaped_no_zombie() {
    let mut evaluator = Evaluator::new();
    run(&mut evaluator, "#[0.05 sleep] &");
    let out = run(&mut evaluator, ".jobs");
    let pid = first_job_pid(&out).expect("jobs output should contain a pid");

    std::thread::sleep(Duration::from_millis(300));
    // .jobs triggers the reaper
    let out = run(&mut evaluator, ".jobs");
    assert!(out.contains("Done"), "job should be Done: {}", out);

    // The child must be fully reaped: /proc/<pid> gone or not in Z state
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
        // stat: "pid (comm) state ..." — state is first char after ") "
        let state = stat
            .rsplit(") ")
            .next()
            .and_then(|rest| rest.chars().next())
            .unwrap_or('?');
        assert_ne!(state, 'Z', "child {} is a zombie: {}", pid, stat);
    }
}

#[test]
fn test_wait_blocks_until_job_finishes() {
    let mut evaluator = Evaluator::new();
    run(&mut evaluator, "#[0.2 sleep] &");

    let start = std::time::Instant::now();
    let output = run(&mut evaluator, "wait");
    let elapsed = start.elapsed();
    let _ = output;

    assert!(
        elapsed >= Duration::from_millis(150),
        "wait should block until the job completes (took {:?})",
        elapsed
    );

    let out = run(&mut evaluator, ".jobs");
    assert!(
        out.contains("Done"),
        "job should be Done after wait: {}",
        out
    );
}

#[test]
fn test_wait_yields_exit_code() {
    let mut evaluator = Evaluator::new();
    run(&mut evaluator, "#[0.05 sleep] &");
    let tokens = lex("wait").unwrap();
    let program = parse(tokens).unwrap();
    let result = evaluator.eval(&program).unwrap();
    assert_eq!(result.exit_code, 0, "wait should yield the job's exit code");
}

#[test]
#[cfg(unix)]
fn test_sigchld_flag_set_on_child_exit() {
    hsab::signals::setup_signal_handlers();
    // Clear any pending flag from other tests' children
    let _ = hsab::signals::check_sigchld();

    let status = std::process::Command::new("sh")
        .args(["-c", "exit 0"])
        .status()
        .expect("spawn sh");
    assert!(status.success());

    // Signal delivery is asynchronous; give it a moment
    let mut flagged = false;
    for _ in 0..50 {
        if hsab::signals::check_sigchld() {
            flagged = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(flagged, "SIGCHLD handler should set the flag when a child exits");
}

#[test]
fn test_reap_jobs_reports_finished_jobs() {
    let mut evaluator = Evaluator::new();
    run(&mut evaluator, "#[0.05 sleep] &");
    std::thread::sleep(Duration::from_millis(300));

    let notices = evaluator.reap_jobs();
    assert_eq!(notices.len(), 1, "one job should have been reaped: {:?}", notices);
    assert!(
        notices[0].contains("Done"),
        "notice should report Done: {}",
        notices[0]
    );

    // Second reap: nothing new
    let notices = evaluator.reap_jobs();
    assert!(notices.is_empty(), "already-reaped job reported again: {:?}", notices);
}
