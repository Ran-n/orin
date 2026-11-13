// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:52.784768
//+ Revised: 	2026/11/12 09:16:53.755495
// -----------------------------------------------------------------------------

use std::{
    fs, thread,
    time::{Duration, Instant},
};

use orin::terminal::{Session, TerminalSnapshot};

fn wait_for(session: &Session, condition: impl Fn(&TerminalSnapshot) -> bool) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = session.snapshot();
        assert!(
            snapshot.error.is_none(),
            "terminal error: {:?}",
            snapshot.error
        );
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "terminal condition timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn has_output(snapshot: &TerminalSnapshot, text: &str) -> bool {
    snapshot.text.lines().any(|line| line.trim() == text)
}

#[test]
fn integration_shell_executes_in_requested_directory_and_preserves_final_output() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = Session::spawn_shell(directory.path(), 24, 100).unwrap();
    session.write(b"echo ORIN_RUNTIME_READY\r").unwrap();
    wait_for(&session, |snapshot| {
        has_output(snapshot, "ORIN_RUNTIME_READY")
    });
    session
        .write(b"echo ORIN_CWD>orin-terminal-cwd.txt\r")
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !directory.path().join("orin-terminal-cwd.txt").exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        fs::read_to_string(directory.path().join("orin-terminal-cwd.txt"))
            .unwrap()
            .trim(),
        "ORIN_CWD"
    );
    session.write(b"exit\r").unwrap();
    let final_snapshot = wait_for(&session, |snapshot| !snapshot.running);
    assert_eq!(final_snapshot.exit_code, Some(0));
    assert!(final_snapshot.text.contains("ORIN_RUNTIME_READY"));
    session.stop().unwrap();
    assert_eq!(session.snapshot().text, final_snapshot.text);
}

#[test]
fn integration_shell_accepts_unicode_input_and_resizes_live_grid() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = Session::spawn_shell(directory.path(), 24, 100).unwrap();
    session
        .write("echo orixe-intelixencia-áéñ\r".as_bytes())
        .unwrap();
    wait_for(&session, |snapshot| {
        has_output(snapshot, "orixe-intelixencia-áéñ")
    });
    session.resize(18, 72).unwrap();
    wait_for(&session, |snapshot| snapshot.rows.len() == 18);
    session.write(b"echo ORIN_RESIZED\r").unwrap();
    wait_for(&session, |snapshot| has_output(snapshot, "ORIN_RESIZED"));
    session.stop().unwrap();
    assert!(!session.snapshot().running);
}

#[test]
fn integration_parallel_sessions_never_share_output() {
    let directory = tempfile::tempdir().unwrap();
    let mut first = Session::spawn_shell(directory.path(), 16, 80).unwrap();
    let mut second = Session::spawn_shell(directory.path(), 16, 80).unwrap();
    first.write(b"echo ORIN_FIRST_SESSION\r").unwrap();
    second.write(b"echo ORIN_SECOND_SESSION\r").unwrap();
    let first_snapshot = wait_for(&first, |snapshot| {
        has_output(snapshot, "ORIN_FIRST_SESSION")
    });
    let second_snapshot = wait_for(&second, |snapshot| {
        has_output(snapshot, "ORIN_SECOND_SESSION")
    });
    assert!(!first_snapshot.text.contains("ORIN_SECOND_SESSION"));
    assert!(!second_snapshot.text.contains("ORIN_FIRST_SESSION"));
    first.stop().unwrap();
    second.stop().unwrap();
}

#[test]
fn integration_stop_is_idempotent_and_disallows_new_input() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = Session::spawn_shell(directory.path(), 12, 60).unwrap();
    session.stop().unwrap();
    session.stop().unwrap();
    assert!(session.write(b"echo cannot-run\r").is_err());
    assert!(!session.snapshot().running);
    session.resize(10, 40).unwrap();
    assert_eq!(session.snapshot().rows.len(), 10);
}

#[test]
fn unit_invalid_cwd_size_and_large_input_are_actionable_errors() {
    let directory = tempfile::tempdir().unwrap();
    assert!(Session::spawn_shell(&directory.path().join("missing"), 12, 60).is_err());
    assert!(Session::spawn_shell(directory.path(), 0, 60).is_err());
    assert!(Session::spawn_shell(directory.path(), 512, 1_024).is_err());
    let mut session = Session::spawn_shell(directory.path(), 12, 60).unwrap();
    assert!(session.resize(12, 0).is_err());
    assert!(session.write(&vec![b'x'; 65_537]).is_err());
    session.write(b"").unwrap();
    session.stop().unwrap();
}
