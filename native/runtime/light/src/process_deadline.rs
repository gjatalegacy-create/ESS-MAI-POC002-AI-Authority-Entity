use std::io::{self, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
#[cfg(windows)]
use std::os::windows::io::{AsHandle, OwnedHandle};

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);
const POLL: Duration = Duration::from_millis(5);
const CLEANUP_BUDGET: Duration = Duration::from_secs(1);
type Capture = Arc<Mutex<Vec<u8>>>;

#[derive(Debug)]
pub(crate) struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct ProcessFailure {
    pub spawn_failed: bool,
    pub timed_out: bool,
    pub elapsed: Duration,
    pub detail: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

struct Worker {
    label: &'static str,
    handle: Option<JoinHandle<io::Result<()>>>,
    #[cfg(windows)]
    pipe: Option<OwnedHandle>,
}

fn snapshot(capture: &Capture) -> Vec<u8> {
    capture.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

fn read_pipe(mut pipe: impl Read, cancel: Arc<AtomicBool>, capture: Capture, limit: usize) -> io::Result<()> {
    let mut chunk = [0u8; 8192];
    while !cancel.load(Ordering::Acquire) {
        match pipe.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let mut bytes = capture.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let keep = n.min(limit.saturating_add(1).saturating_sub(bytes.len()));
                bytes.extend_from_slice(&chunk[..keep]);
            }
            Err(error) if cancel.load(Ordering::Acquire) => { let _ = error; return Ok(()); }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn write_pipe(mut pipe: impl Write, input: Vec<u8>, cancel: Arc<AtomicBool>) -> io::Result<()> {
    let mut offset = 0;
    while offset < input.len() && !cancel.load(Ordering::Acquire) {
        match pipe.write(&input[offset..]) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "stdin write returned zero")),
            Ok(n) => offset += n,
            Err(error) if cancel.load(Ordering::Acquire) => { let _ = error; return Ok(()); }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL),
            Err(error) => return Err(error),
        }
    }
    // ChildStdin is unbuffered. Dropping it supplies EOF without an unbounded flush.
    Ok(())
}

fn reap_workers(workers: &mut [Worker]) -> Option<String> {
    let mut failure = None;
    for worker in workers {
        if worker.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(handle) = worker.handle.take() {
                match handle.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => failure = Some(format!("{}: {error}", worker.label)),
                    Err(_) => failure = Some(format!("{} worker panicked", worker.label)),
                }
                #[cfg(windows)]
                { worker.pipe.take(); }
            }
        }
    }
    failure
}

#[cfg(windows)]
fn cancel_worker(worker: &Worker) {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" {
        fn CancelSynchronousIo(thread: *mut std::ffi::c_void) -> i32;
        fn CancelIoEx(pipe: *mut std::ffi::c_void, overlapped: *mut std::ffi::c_void) -> i32;
    }
    if let Some(handle) = &worker.handle {
        // The handle remains owned until the worker finishes; cancellation is repeated
        // because a worker may cross the cancel check just before entering synchronous I/O.
        unsafe { CancelSynchronousIo(handle.as_raw_handle()); }
        if let Some(pipe) = &worker.pipe {
            // std may issue overlapped pipe I/O. The owned duplicate keeps the
            // file object alive even while the worker finishes and closes its handle.
            unsafe { CancelIoEx(pipe.as_raw_handle(), std::ptr::null_mut()); }
        }
    }
}

#[cfg(not(windows))]
fn cancel_worker(_: &Worker) {}

#[cfg(unix)]
fn prepare_pipe(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    extern "C" { fn fcntl(fd: i32, command: i32, ...) -> i32; }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const NONBLOCK: i32 = 0x800;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const NONBLOCK: i32 = 4;
    let fd = pipe.as_raw_fd();
    let flags = unsafe { fcntl(fd, 3) };
    if flags == -1 || unsafe { fcntl(fd, 4, flags | NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn prepare_pipe<T>(_: &T) -> io::Result<()> { Ok(()) }

fn cleanup(child: &mut Child, workers: &mut [Worker], cancel: &AtomicBool) -> String {
    cancel.store(true, Ordering::Release);
    let kill = match child.try_wait() {
        Ok(Some(_)) => "already_exited".to_owned(),
        _ => format!("{:?}", child.kill()),
    };
    let started = Instant::now();
    let mut reaped = false;
    let mut last_error = None;
    loop {
        for worker in workers.iter() { cancel_worker(worker); }
        if let Some(error) = reap_workers(workers) { last_error = Some(error); }
        match child.try_wait() {
            Ok(Some(_)) => reaped = true,
            Err(error) => last_error = Some(format!("child try_wait: {error}")),
            Ok(None) => {}
        }
        if reaped && workers.iter().all(|worker| worker.handle.is_none()) { break; }
        if started.elapsed() >= CLEANUP_BUDGET { break; }
        thread::sleep(POLL);
    }
    let pending = workers.iter().filter(|worker| worker.handle.is_some()).count();
    format!("direct_child_kill={kill}; child_reaped={reaped}; pending_io_workers={pending}; cleanup_complete={}; cleanup_error={last_error:?}", reaped && pending == 0)
}

/// One transport budget covers input delivery, child exit and both output EOFs.
/// Failure never establishes rollback or permits a fresh retry.
pub(crate) fn run(command: &mut Command, input: Vec<u8>, timeout: Duration, limit: usize) -> Result<ProcessOutput, ProcessFailure> {
    let started = Instant::now();
    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stderr = Arc::new(Mutex::new(Vec::new()));
    let cancel = Arc::new(AtomicBool::new(false));
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|error| ProcessFailure {
            spawn_failed: true, timed_out: false, elapsed: started.elapsed(),
            detail: error.to_string(), stdout: Vec::new(), stderr: Vec::new(),
        })?;
    let mut workers = Vec::new();
    let setup = (|| -> io::Result<()> {
        let input_pipe = child.stdin.take().ok_or_else(|| io::Error::other("stdin pipe missing"))?;
        let output_pipe = child.stdout.take().ok_or_else(|| io::Error::other("stdout pipe missing"))?;
        let error_pipe = child.stderr.take().ok_or_else(|| io::Error::other("stderr pipe missing"))?;
        prepare_pipe(&input_pipe)?; prepare_pipe(&output_pipe)?; prepare_pipe(&error_pipe)?;
        #[cfg(windows)]
        let input_cancel = input_pipe.as_handle().try_clone_to_owned()?;
        #[cfg(windows)]
        let output_cancel = output_pipe.as_handle().try_clone_to_owned()?;
        #[cfg(windows)]
        let error_cancel = error_pipe.as_handle().try_clone_to_owned()?;
        let c = Arc::clone(&cancel); let capture = Arc::clone(&stdout);
        workers.push(Worker { label: "stdout", #[cfg(windows)] pipe: Some(output_cancel), handle: Some(thread::Builder::new().name("process-stdout".into())
            .spawn(move || read_pipe(output_pipe, c, capture, limit))?) });
        let c = Arc::clone(&cancel); let capture = Arc::clone(&stderr);
        workers.push(Worker { label: "stderr", #[cfg(windows)] pipe: Some(error_cancel), handle: Some(thread::Builder::new().name("process-stderr".into())
            .spawn(move || read_pipe(error_pipe, c, capture, limit))?) });
        let c = Arc::clone(&cancel);
        workers.push(Worker { label: "stdin", #[cfg(windows)] pipe: Some(input_cancel), handle: Some(thread::Builder::new().name("process-stdin".into())
            .spawn(move || write_pipe(input_pipe, input, c))?) });
        Ok(())
    })();
    let mut status = None;
    let (timed_out, reason) = match setup {
        Err(error) => (false, format!("I/O setup: {error}")),
        Ok(()) => loop {
            if started.elapsed() >= timeout { break (true, "transport deadline exceeded".to_owned()); }
            if let Some(error) = reap_workers(&mut workers) { break (false, error); }
            match child.try_wait() {
                Ok(Some(exit)) => status = Some(exit),
                Err(error) => break (false, format!("child try_wait: {error}")),
                Ok(None) => {}
            }
            if workers.iter().all(|worker| worker.handle.is_none()) {
                if let Some(status) = status {
                    return Ok(ProcessOutput { status, stdout: snapshot(&stdout), stderr: snapshot(&stderr) });
                }
            }
            thread::sleep(POLL);
        },
    };
    let cleanup = cleanup(&mut child, &mut workers, &cancel);
    Err(ProcessFailure {
        spawn_failed: false, timed_out, elapsed: started.elapsed(), detail: format!("{reason}; {cleanup}"),
        stdout: snapshot(&stdout), stderr: snapshot(&stderr),
    })
}

/// File-based project requests keep their existing null stdin and inherited diagnostics.
pub(crate) fn run_status(command: &mut Command, timeout: Duration) -> Result<ExitStatus, ProcessFailure> {
    let started = Instant::now();
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit())
        .spawn().map_err(|error| ProcessFailure {
            spawn_failed: true, timed_out: false, elapsed: started.elapsed(),
            detail: error.to_string(), stdout: Vec::new(), stderr: Vec::new(),
        })?;
    let (timed_out, reason) = loop {
        if started.elapsed() >= timeout { break (true, "transport deadline exceeded".to_owned()); }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => thread::sleep(POLL),
            Err(error) => break (false, format!("child try_wait: {error}")),
        }
    };
    let cleanup = cleanup(&mut child, &mut [], &AtomicBool::new(false));
    Err(ProcessFailure {
        spawn_failed: false, timed_out, elapsed: started.elapsed(), detail: format!("{reason}; {cleanup}"),
        stdout: Vec::new(), stderr: Vec::new(),
    })
}
