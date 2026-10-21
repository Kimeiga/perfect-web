//! A bounded observation of ADR-0205's explicit small-stack property.
//! Only a nonce-bound drop phase ending in the native stack-overflow abort
//! is a negative property observation. Other failures are invalid evidence.
use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ID: &str = "PW_GRAPH_STACK_PROBE";
const PREFIX: &str = "PW_GRAPH_STACK ";

pub struct Observation {
    pub overflow: bool,
    pub values: Vec<usize>,
    pub output: String,
}

fn marker(nonce: &str, phase: &str) {
    println!("\n{PREFIX}{nonce} {phase}");
    std::io::stdout().flush().expect("probe output is writable");
}

#[cfg(unix)]
fn aborted(status: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(6)
}
#[cfg(not(unix))]
fn aborted(_: ExitStatus) -> bool {
    false
}

pub fn classify(
    nonce: &str,
    name: &str,
    fields: usize,
    status: ExitStatus,
    stdout: String,
    stderr: String,
) -> Result<Observation, String> {
    let output = format!("child status: {status}\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with(PREFIX))
        .collect();
    let ready = format!("{PREFIX}{nonce} ready {name}");
    let values_prefix = format!("{PREFIX}{nonce} values ");
    let drop_phase = format!("{PREFIX}{nonce} drop {name}");
    let complete = format!("{PREFIX}{nonce} complete {name}");
    let parsed = lines
        .get(1)
        .and_then(|line| line.strip_prefix(&values_prefix))
        .and_then(|line| {
            line.split(',')
                .map(str::parse::<usize>)
                .collect::<Result<Vec<_>, _>>()
                .ok()
        });
    let complete_protocol = lines.len() == 4 && lines[3] == complete;
    let drop_protocol = lines.len() == 3;
    if lines.first() != Some(&ready.as_str())
        || lines.get(2) != Some(&drop_phase.as_str())
        || parsed.as_ref().is_none_or(|values| {
            fields < 2 || values.len() != fields || values.iter().skip(1).any(|value| *value > 1)
        })
    {
        return Err(format!("INVALID graph stack probe protocol\n{output}"));
    }
    let error_lines: Vec<_> = stderr.lines().filter(|line| !line.is_empty()).collect();
    let native_overflow = error_lines.len() == 2
        && error_lines[0].starts_with("thread 'graph-stack-contract'")
        && error_lines[0].ends_with("has overflowed its stack")
        && error_lines[1] == "fatal runtime error: stack overflow, aborting";
    let overflow = drop_protocol && aborted(status) && native_overflow;
    if !(complete_protocol && status.success() && stderr.is_empty()) && !overflow {
        return Err(format!("INVALID graph stack probe termination\n{output}"));
    }
    Ok(Observation {
        overflow,
        values: parsed.expect("checked values"),
        output,
    })
}

/// The child stays in its caller's owned process group. The callback prepares
/// the exact deep values and observations; only their explicit drop follows
/// the phase receipt. The native producer must still reject any memory stop.
pub fn observe(
    name: &'static str,
    fields: usize,
    prepare: impl FnOnce() -> (Vec<usize>, Vec<pw_render::Value>) + Send + 'static,
) -> Result<Option<Observation>, String> {
    observe_bounded(name, fields, Duration::from_secs(30), prepare)
}

pub fn observe_bounded(
    name: &'static str,
    fields: usize,
    timeout: Duration,
    prepare: impl FnOnce() -> (Vec<usize>, Vec<pw_render::Value>) + Send + 'static,
) -> Result<Option<Observation>, String> {
    if let Ok(nonce) = std::env::var(ID) {
        if nonce.is_empty() || !nonce.bytes().all(|c| c.is_ascii_digit() || c == b'-') {
            return Err("INVALID graph stack probe invocation identity".into());
        }
        marker(&nonce, &format!("ready {name}"));
        let worker = std::thread::Builder::new()
            .name("graph-stack-contract".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                let (values, held) = prepare();
                marker(
                    &nonce,
                    &format!(
                        "values {}",
                        values
                            .iter()
                            .map(usize::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                );
                marker(&nonce, &format!("drop {name}"));
                drop(held);
                marker(&nonce, &format!("complete {name}"));
            })
            .map_err(|error| format!("INVALID graph stack probe thread: {error}"))?;
        worker
            .join()
            .map_err(|_| "INVALID graph stack probe worker panic".to_string())?;
        return Ok(None);
    }
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    );
    let mut child = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(ID, &nonce)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("INVALID graph stack probe launch: {error}"))?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .map_err(|error| format!("INVALID graph stack probe reap: {error}"))?;
                return Err(format!(
                    "INVALID graph stack probe timeout/wait: {result:?}; child status: {}\nstdout:\n{}\nstderr:\n{}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .stdout
        .take()
        .expect("owned stdout")
        .read_to_string(&mut stdout)
        .map_err(|error| format!("INVALID graph stack stdout: {error}"))?;
    child
        .stderr
        .take()
        .expect("owned stderr")
        .read_to_string(&mut stderr)
        .map_err(|error| format!("INVALID graph stack stderr: {error}"))?;
    classify(&nonce, name, fields, status, stdout, stderr).map(Some)
}
