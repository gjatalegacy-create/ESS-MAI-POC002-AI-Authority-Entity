#[path = "../src/process_deadline.rs"]
mod process_deadline;

use std::{process::Command, time::{Duration, Instant}};

fn child(mode: &str) -> Command {
    static FIXTURE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let executable = FIXTURE.get_or_init(|| {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("aigovern-process-test-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("child.rs");
        std::fs::write(&source, include_str!("fixtures/process_deadline_child.rs")).unwrap();
        let executable = dir.join(if cfg!(windows) { "child.exe" } else { "child" });
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(compiler).arg("--edition=2021").arg(&source).arg("-o").arg(&executable).output().unwrap();
        assert!(output.status.success(), "fixture compiler: {}", String::from_utf8_lossy(&output.stderr));
        executable
    });
    let mut cmd = Command::new(executable);
    cmd.env("PROCESS_FIXTURE_WAIT_MS", "3000");
    cmd.env("PROCESS_FIXTURE_MODE", mode);
    cmd
}

#[test]
fn blocked_stdin_obeys_deadline_and_retains_output() {
    let mut command = child("no_read");
    let start = Instant::now();
    let e = process_deadline::run(&mut command, vec![b'x'; 65536], Duration::from_millis(750), 1024).unwrap_err();
    assert!(e.timed_out && !e.spawn_failed, "{e:?}");
    assert!(e.detail.contains("cleanup_complete=true"), "{e:?}");
    assert!(start.elapsed() < Duration::from_secs(3));
    assert!(e.elapsed >= Duration::from_millis(750));
    assert!(String::from_utf8_lossy(&e.stdout).contains("retained-before-wait"));
    assert!(String::from_utf8_lossy(&e.stderr).contains("stderr-before-wait"));
}

#[test]
fn descendant_pipe_does_not_outlive_deadline_workers() {
    let mut command = child("descendant");
    let start = Instant::now();
    let e = process_deadline::run(&mut command, Vec::new(), Duration::from_millis(750), 1024).unwrap_err();
    assert!(e.timed_out, "{e:?}");
    assert!(e.detail.contains("cleanup_complete=true"), "{e:?}");
    assert!(e.detail.contains("pending_io_workers=0"), "{e:?}");
    assert!(String::from_utf8_lossy(&e.stdout).contains("parent-exited"));
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn project_without_input_obeys_deadline() {
    let e = process_deadline::run_status(&mut child("project"), Duration::from_millis(750)).unwrap_err();
    assert!(e.timed_out && e.detail.contains("cleanup_complete=true"), "{e:?}");
}

#[test]
fn valid_payload_and_output_are_byte_exact() {
    let bytes = b"line one,\0\r\nline two;=\\\xff".repeat(8192);
    let out = process_deadline::run(&mut child("echo"), bytes.clone(), process_deadline::DEFAULT_TIMEOUT, bytes.len()+1).unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, bytes);
    assert_eq!(out.stderr, b"exact-stderr\0,\n");
}

#[test]
fn nonzero_status_and_its_evidence_are_preserved() {
    let out = process_deadline::run(&mut child("nonzero"), b"evidence".to_vec(), Duration::from_secs(2), 1024).unwrap();
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(out.stdout, b"evidence");
    assert_eq!(out.stderr, b"exact-stderr\0,\n");
}

#[test]
fn output_is_bounded_while_pipes_keep_draining() {
    let out = process_deadline::run(&mut child("flood"), Vec::new(), Duration::from_secs(2), 1024).unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, vec![b'x'; 1025]);
    assert_eq!(out.stderr, vec![b'y'; 1025]);
}

#[test]
fn missing_binary_is_spawn_failure() {
    let e = process_deadline::run(&mut Command::new("missing-aigovern-process-fixture-20261029"), Vec::new(), Duration::from_secs(2), 1024).unwrap_err();
    assert!(e.spawn_failed && !e.timed_out);
    assert!(e.stdout.is_empty() && e.stderr.is_empty());
}
