//! The `sail-bench` binary's command surface, as `sail bench` delegates to it.

use std::process::Command;

#[test]
fn help_lists_the_benchmark_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_sail-bench"))
        .arg("--help")
        .output();
    let Ok(output) = output else {
        unreachable!("cannot run sail-bench: {output:?}")
    };
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("history-replay") && help.contains("agent-loop"),
        "{help}"
    );
}
