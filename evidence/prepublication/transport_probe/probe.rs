#[path = "C:/Users/AG/Desktop/ess-mai-project/poc/poc-002-ai-authority-entity/native/runtime/light/src/process_deadline.rs"]
mod process_deadline;

use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() >= 3, "probe EXECUTABLE CASE [MARKERS]");
    let mut command = Command::new(&args[1]);
    command.env("PROCESS_FIXTURE_MODE", "no_read")
        .env("PROCESS_FIXTURE_WAIT_MS", "3000");
    if let Some(markers) = args.get(3) {
        command.env("DIAGNOSTIC_MARKERS", markers);
    }
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let start = Instant::now();
    println!("CASE={} START_EPOCH_NS={epoch} TIMEOUT_MS=750 INPUT_BYTES=65536 CAPTURE_LIMIT=1024", args[2]);
    match process_deadline::run(&mut command, vec![b'x'; 65536], Duration::from_millis(750), 1024) {
        Ok(output) => println!("UNEXPECTED_SUCCESS {output:?}"),
        Err(error) => {
            println!("WALL_ELAPSED_NS={}", start.elapsed().as_nanos());
            println!("STDOUT_LEN={} STDERR_LEN={}", error.stdout.len(), error.stderr.len());
            println!("STDOUT_BYTES={:?}", error.stdout);
            println!("STDERR_BYTES={:?}", error.stderr);
            println!("STDOUT_MARKER={} STDERR_MARKER={}",
                String::from_utf8_lossy(&error.stdout).contains("retained-before-wait"),
                String::from_utf8_lossy(&error.stderr).contains("stderr-before-wait"));
            println!("FULL_FAILURE={error:?}");
        }
    }
    if let Some(markers) = args.get(3) {
        println!("CHILD_MARKERS={:?}", std::fs::read_to_string(markers));
    }
}
