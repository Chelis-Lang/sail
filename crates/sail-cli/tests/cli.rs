//! Smoke tests for the `sail` binary's command surface.

use std::process::{Command, Output};

fn sail(args: &[&str]) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_sail")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run sail: {e}"),
    }
}

#[test]
fn help_lists_every_charter_command() {
    let output = sail(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    for cmd in ["verify", "watch", "serve", "status", "cache", "bench"] {
        assert!(text.contains(cmd), "help is missing `{cmd}`:\n{text}");
    }
}

#[test]
fn unimplemented_commands_exit_3_with_stable_code() {
    for cmd in ["bench"] {
        let output = sail(&[cmd]);
        assert_eq!(output.status.code(), Some(3), "sail {cmd}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("SAIL0001"));
    }
}

#[test]
fn unknown_command_is_a_usage_error() {
    let output = sail(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SAIL0002"));
}

#[test]
fn no_arguments_is_a_usage_error() {
    assert_eq!(sail(&[]).status.code(), Some(2));
}
