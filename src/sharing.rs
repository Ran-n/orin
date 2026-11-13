// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 22:51:38.450960
//+ Revised: 	2026/11/12 09:16:39.494387
// -----------------------------------------------------------------------------

//! Bounded retries for transient Windows file-sharing conflicts.
//!
//! Replacing a file that another handle (a reader, an agent, an antivirus scan) has open fails on
//! Windows with "Access is denied" or a sharing violation until that handle closes, usually within
//! milliseconds. Other platforms never report these errors for that reason, so they never retry.

use std::{io, thread, time::Duration};

const ATTEMPTS: u32 = 8;
const MAX_DELAY: Duration = Duration::from_millis(50);

/// Run `operation`, retrying it with a short backoff (at most about 0.3 s in total) while it fails with
/// a Windows sharing conflict. Any other error, or the last conflict, is returned unchanged.
pub(crate) fn retry<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempt = 1;
    loop {
        match operation() {
            Err(error) if attempt < ATTEMPTS && is_sharing_conflict(&error) => {
                tracing::debug!(target: "orin::sharing", attempt, %error, "file busy; retrying");
                thread::sleep(delay(attempt));
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn delay(attempt: u32) -> Duration {
    Duration::from_millis(5u64 << attempt.min(4)).min(MAX_DELAY)
}

fn is_sharing_conflict(error: &io::Error) -> bool {
    // ERROR_ACCESS_DENIED (5), ERROR_SHARING_VIOLATION (32), ERROR_LOCK_VIOLATION (33).
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn denied() -> io::Error {
        io::Error::from_raw_os_error(5)
    }

    #[test]
    fn unit_success_runs_once() {
        let mut calls = 0;
        assert_eq!(
            retry(|| {
                calls += 1;
                Ok(7)
            })
            .unwrap(),
            7
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn unit_unrelated_errors_are_returned_without_retrying() {
        let mut calls = 0;
        let error = retry::<()>(|| {
            calls += 1;
            Err(io::ErrorKind::NotFound.into())
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(calls, 1);
    }

    #[cfg(windows)]
    #[test]
    fn unit_transient_conflict_recovers() {
        let mut calls = 0;
        let value = retry(|| {
            calls += 1;
            if calls < 3 {
                Err(denied())
            } else {
                Ok("saved")
            }
        })
        .unwrap();
        assert_eq!((value, calls), ("saved", 3));
    }

    #[cfg(windows)]
    #[test]
    fn unit_persistent_conflict_stops_after_bounded_attempts() {
        let mut calls = 0;
        let error = retry::<()>(|| {
            calls += 1;
            Err(denied())
        })
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(5));
        assert_eq!(calls, ATTEMPTS);
    }

    #[cfg(unix)]
    #[test]
    fn unit_unix_never_retries_permission_errors() {
        let mut calls = 0;
        assert!(
            retry::<()>(|| {
                calls += 1;
                Err(denied())
            })
            .is_err()
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn unit_backoff_is_capped() {
        let total: Duration = (1..ATTEMPTS).map(delay).sum();
        assert!(total <= Duration::from_millis(300), "{total:?}");
        assert!((1..ATTEMPTS).all(|attempt| delay(attempt) <= MAX_DELAY));
    }
}
