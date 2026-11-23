//! Run helper programs with a time limit, so a stuck printer queue or a slow helper can never
//! keep the app waiting forever.
use anyhow::{Context, Result, bail};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// How long a printer query (lpstat, lpoptions) may take before it is abandoned.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(8);
/// How long handing a job to the print system may take.
pub const SUBMIT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long rendering one document page may take.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(60);

/// Run `cmd` and collect its output. When it does not finish within `limit`, it is stopped
/// and an error says which program did not answer.
pub fn output_within(cmd: &mut Command, limit: Duration) -> Result<Output> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Could not start {program}"))?;
    // Read both pipes on their own threads so a chatty program cannot fill a pipe and stall.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "{program} did not answer within {} seconds",
                limit.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ok(Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_program_that_finishes_returns_its_output() {
        let o = output_within(Command::new("echo").arg("hello"), Duration::from_secs(5)).unwrap();
        assert!(o.status.success());
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "hello");
    }

    #[test]
    fn a_program_that_hangs_is_stopped_in_time() {
        let start = Instant::now();
        let error = output_within(Command::new("sleep").arg("30"), Duration::from_millis(300))
            .unwrap_err()
            .to_string();
        assert!(error.contains("did not answer"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_missing_program_is_an_error() {
        assert!(
            output_within(
                &mut Command::new("thermalprint-no-such-program"),
                Duration::from_secs(1)
            )
            .is_err()
        );
    }
}
