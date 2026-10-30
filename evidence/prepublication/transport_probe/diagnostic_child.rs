
use std::{io::{Read, Write}, process::{Command, Stdio}, time::Duration};

fn diagnostic_mark(label: &str) {
    if let Ok(path) = std::env::var("DIAGNOSTIC_MARKERS") {
        let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{} {} {}", label, ns, std::process::id());
            let _ = f.flush();
        }
    }
}

fn main() {
    diagnostic_mark("MAIN_ENTERED");
    if let Ok(path) = std::env::var("PROCESS_PID_LOG") {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(f, "{}", std::process::id()).unwrap();
    }
    let mode = std::env::args().nth(1).filter(|s| !s.starts_with("--"))
        .unwrap_or_else(|| std::env::var("PROCESS_FIXTURE_MODE").unwrap());
    match mode.as_str() {
        "no_read" | "project" | "hold" => {
            println!("retained-before-wait"); std::io::stdout().flush().unwrap(); diagnostic_mark("STDOUT_FLUSHED");
            eprintln!("stderr-before-wait"); diagnostic_mark("STDERR_WRITTEN");
            std::thread::sleep(Duration::from_millis(std::env::var("PROCESS_FIXTURE_WAIT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(10000)));
        }
        "descendant" => {
            let mut input = Vec::new(); std::io::stdin().read_to_end(&mut input).unwrap();
            Command::new(std::env::current_exe().unwrap()).arg("hold")
                .stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
            println!("parent-exited");
        }
        "echo" | "nonzero" => {
            let mut input = Vec::new(); std::io::stdin().read_to_end(&mut input).unwrap();
            std::io::stdout().write_all(&input).unwrap();
            std::io::stderr().write_all(b"exact-stderr\0,\n").unwrap();
            if mode == "nonzero" { std::process::exit(7); }
        }
        "flood" => {
            std::io::stdout().write_all(&vec![b'x'; 131072]).unwrap();
            std::io::stderr().write_all(&vec![b'y'; 131072]).unwrap();
        }
        _ => panic!("unexpected fixture mode"),
    }
}
