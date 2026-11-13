// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 15:04:14.375749
//+ Revised: 	2026/11/12 15:11:30.798783
// -----------------------------------------------------------------------------

use orin::{background::Workers, keyboard, startup::Options};
use proptest::prelude::*;
use std::{ffi::OsString, sync::mpsc, time::Duration};

#[test]
fn contract_startup_parses_paths_switches_and_rejects_incomplete_arguments() {
    let parse = |args: &[&str]| Options::parse(args.iter().map(OsString::from));
    let options = parse(&[
        "--project",
        "folder",
        "--settings",
        "workspace.json",
        "--test-driver",
    ])
    .unwrap();
    assert_eq!(options.project.unwrap(), std::path::Path::new("folder"));
    assert_eq!(
        options.settings.unwrap(),
        std::path::Path::new("workspace.json")
    );
    assert!(options.test_driver);
    for arg in ["--help", "-h"] {
        assert!(parse(&[arg]).unwrap().help);
    }
    for arg in ["--version", "-V"] {
        assert!(parse(&[arg]).unwrap().version);
    }
    for args in [
        vec!["--unknown"],
        vec!["--project"],
        vec!["--settings", ""],
        vec!["--project", "--help"],
        vec!["--test-driver"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    assert_eq!(parse(&[]).unwrap(), Options::default());
}

#[test]
fn integration_workers_run_concurrently_drain_on_shutdown_and_refuse_closed_queue() {
    let mut workers = Workers::new(2, 2).unwrap();
    let (started, starts) = mpsc::channel();
    let (release, released) = mpsc::channel();
    workers
        .submit(move || {
            started.send(()).unwrap();
            released.recv().unwrap();
        })
        .unwrap();
    starts.recv_timeout(Duration::from_secs(5)).unwrap();
    let (other, completed) = mpsc::channel();
    workers.submit(move || other.send(42).unwrap()).unwrap();
    assert_eq!(completed.recv_timeout(Duration::from_secs(5)).unwrap(), 42);
    release.send(()).unwrap();
    workers.shutdown();
    assert!(workers.submit(|| {}).is_err());
    workers.shutdown();
}

#[test]
fn integration_busy_worker_queue_is_bounded_and_recovers_after_a_panicking_job() {
    let mut workers = Workers::new(1, 1).unwrap();
    let (started, starts) = mpsc::channel();
    let (release, released) = mpsc::channel();
    workers
        .submit(move || {
            started.send(()).unwrap();
            released.recv().unwrap();
        })
        .unwrap();
    starts.recv_timeout(Duration::from_secs(5)).unwrap();
    let (panicking, panics) = mpsc::channel();
    workers
        .submit(move || {
            panicking.send(()).unwrap();
            panic!("intentional worker failure");
        })
        .unwrap();
    assert!(workers.submit(|| {}).is_err());
    release.send(()).unwrap();
    panics.recv_timeout(Duration::from_secs(5)).unwrap();
    let (recovered, recovery) = mpsc::channel();
    workers.submit(move || recovered.send(42).unwrap()).unwrap();
    assert_eq!(recovery.recv_timeout(Duration::from_secs(5)).unwrap(), 42);
    // shutdown joins every accepted job, including the caught panic.
    workers.shutdown();
}

#[test]
fn contract_terminal_key_sequences_and_modifiers() {
    for (key, expected) in [
        ("enter", "\r"),
        ("backspace", "\x7f"),
        ("escape", "\x1b"),
        ("tab", "\t"),
        ("home", "\x1b[H"),
        ("end", "\x1b[F"),
        ("delete", "\x1b[3~"),
        ("insert", "\x1b[2~"),
        ("pageup", "\x1b[5~"),
        ("pagedown", "\x1b[6~"),
        ("f1", "\x1bOP"),
        ("f2", "\x1bOQ"),
        ("f3", "\x1bOR"),
        ("f4", "\x1bOS"),
        ("f5", "\x1b[15~"),
        ("f6", "\x1b[17~"),
        ("f7", "\x1b[18~"),
        ("f8", "\x1b[19~"),
        ("f9", "\x1b[20~"),
        ("f10", "\x1b[21~"),
        ("f11", "\x1b[23~"),
        ("f12", "\x1b[24~"),
    ] {
        assert_eq!(
            keyboard::encode(key, false, false, false, false).unwrap(),
            expected.as_bytes()
        );
    }
    for (key, code) in [
        ("up", b'A'),
        ("down", b'B'),
        ("right", b'C'),
        ("left", b'D'),
    ] {
        for application in [false, true] {
            assert_eq!(
                keyboard::encode(key, false, false, false, application).unwrap(),
                vec![27, if application { b'O' } else { b'[' }, code]
            );
        }
    }
    for (key, code) in [("a", 1), ("Z", 26), ("@", 0), (" ", 0), ("?", 127)] {
        assert_eq!(
            keyboard::encode(key, true, true, false, false).unwrap(),
            vec![27, code]
        );
    }
    assert_eq!(
        keyboard::encode("tab", false, false, true, false).unwrap(),
        b"\x1b[Z"
    );
    assert!(keyboard::encode("unknown", false, false, false, false).is_none());
    assert!(keyboard::encode("1", true, false, false, false).is_none());
    assert_eq!(keyboard::paste("a\r\nb\rc\x1bd", false), b"a\nb\ncd");
    assert_eq!(keyboard::paste("text", true), b"\x1b[200~text\x1b[201~");
}

proptest! {
    #[test]
    fn property_paste_never_injects_escape_or_carriage_return(text in ".*") {
        let bytes = keyboard::paste(&text, false);
        prop_assert!(!bytes.contains(&27));
        prop_assert!(!bytes.contains(&b'\r'));
        let mut bracketed = b"\x1b[200~".to_vec();
        bracketed.extend(&bytes);
        bracketed.extend(b"\x1b[201~");
        prop_assert_eq!(keyboard::paste(&text, true), bracketed);
    }
}
