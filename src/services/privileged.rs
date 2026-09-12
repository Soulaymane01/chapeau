use crate::errors::{ChapeauError, Result};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Default budget for long privileged operations (package removal).
pub(crate) const REMOVAL_TIMEOUT: Duration = Duration::from_secs(1800);
/// Default budget for short privileged operations (service control).
pub(crate) const SERVICE_TIMEOUT: Duration = Duration::from_secs(120);

/// Run a privileged command safely:
///
/// - stdin is detached, so a graphical authorization agent is used instead of
///   a text prompt trying to read our terminal;
/// - a timeout guarantees the caller (and the UI waiting on it) can never hang
///   forever if authorization is denied or the child wedges.
pub(crate) fn run(
    program: &str,
    extra_args: &[&str],
    args: &[String],
    timeout: Duration,
) -> Result<Output> {
    let mut command = Command::new(program);
    command
        .args(extra_args)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let deadline = Instant::now() + timeout;

    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ChapeauError::Backend(format!(
                "the privileged command did not finish within {}s (authorization may have been denied)",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    child.wait_with_output().map_err(Into::into)
}
