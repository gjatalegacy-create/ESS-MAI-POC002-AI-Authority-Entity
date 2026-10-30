use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let start_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let started = Instant::now();
    let mut child = Command::new(&args[1])
        .env("PROCESS_FIXTURE_MODE", "no_read").env("PROCESS_FIXTURE_WAIT_MS", "3000")
        .env("DIAGNOSTIC_MARKERS", &args[2])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let spawn_elapsed = started.elapsed();
    println!("START_EPOCH_NS={start_epoch} SPAWN_ELAPSED_NS={}", spawn_elapsed.as_nanos());
    if let Some(remaining) = Duration::from_millis(750).checked_sub(started.elapsed()) {
        std::thread::sleep(remaining);
    }
    println!("BEFORE_KILL_ELAPSED_NS={}", started.elapsed().as_nanos());
    println!("MARKERS_BEFORE_KILL={:?}", std::fs::read_to_string(&args[2]));
    println!("KILL_RESULT={:?}", child.kill());
    println!("WAIT_RESULT={:?}", child.wait());
    let mut out = Vec::new(); let mut err = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
    child.stderr.take().unwrap().read_to_end(&mut err).unwrap();
    println!("STDOUT_BYTES={out:?} STDERR_BYTES={err:?}");
    println!("MARKERS_AFTER_REAP={:?}", std::fs::read_to_string(&args[2]));
}
