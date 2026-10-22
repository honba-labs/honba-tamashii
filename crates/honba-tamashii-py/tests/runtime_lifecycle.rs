//! The explicit runtime end to end: lifecycle, no leaked threads, and an
//! idempotent stop. One test function, because the runtime slot and the
//! process thread count are global.

use std::path::PathBuf;

use _honba::runtime::{info, is_running, start, stop};

fn thread_count() -> Option<usize> {
    std::fs::read_dir("/proc/self/task").ok().map(|d| d.count())
}

fn data_dir() -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("honba-py-runtime-lifecycle");
    std::fs::create_dir_all(&dir).expect("data dir");
    dir.to_string_lossy().into_owned()
}

#[test]
fn the_runtime_starts_once_stops_cleanly_and_leaves_no_threads() {
    let before = thread_count();

    // A fresh process has no runtime.
    assert!(!is_running());
    assert!(info().is_none());

    let started = start(None).expect("the runtime starts");
    assert_eq!(started.flavor, "multi-thread");
    assert!(started.worker_threads >= 1);
    assert!(is_running());

    // A second start is refused, not replaced.
    assert!(start(None).is_err());
    assert!(is_running());

    // The description is stable while it runs.
    let described = info().expect("the runtime is described");
    assert_eq!(described.flavor, "multi-thread");
    assert_eq!(described.generation, started.generation);

    assert!(stop(), "stopping a running runtime reports it");
    assert!(!is_running());
    assert!(info().is_none());

    // Stopping again is a no-op, not an error.
    assert!(!stop());

    // The runtime can be started again, with a new generation.
    let restarted = start(Some(1)).expect("the runtime restarts");
    assert!(restarted.generation > started.generation);
    assert!(stop());

    let after = thread_count();
    if let (Some(before), Some(after)) = (before, after) {
        assert!(
            after <= before + 1,
            "the runtime leaked threads: {before} -> {after}"
        );
    }

    // The data directory helper still works (the runtime used to serve the
    // in-process REST router from it).
    assert!(!data_dir().is_empty());
}
