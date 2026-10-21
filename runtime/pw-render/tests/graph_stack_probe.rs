//! Invalid neighbors for the explicit deep-drop property observation.
#[path = "graphs/stack_probe.rs"]
mod stack_probe;
use std::time::Duration;

#[cfg(unix)]
fn status(raw: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(raw)
}

#[test]
#[cfg(unix)]
fn only_the_complete_property_or_identified_native_overflow_is_observed() {
    let prefix = "PW_GRAPH_STACK 123-456 ";
    let drop = format!("{prefix}ready property\n{prefix}values 100000,1\n{prefix}drop property\n");
    let complete = format!("{drop}{prefix}complete property\n");
    let overflow = "\nthread 'graph-stack-contract' (123) has overflowed its stack\nfatal runtime error: stack overflow, aborting\n";
    let done = stack_probe::classify(
        "123-456",
        "property",
        2,
        status(0),
        complete.clone(),
        String::new(),
    )
    .unwrap();
    assert!(!done.overflow);
    assert_eq!(done.values, vec![100000, 1]);
    let failed = stack_probe::classify(
        "123-456",
        "property",
        2,
        status(6),
        drop.clone(),
        overflow.into(),
    )
    .unwrap();
    assert!(failed.overflow);
    assert!(failed.output.contains("child status:"));
    for (native, out, err) in [
        (0, drop.clone(), overflow.to_string()),
        (0, drop.clone(), String::new()),
        (9, drop.clone(), overflow.to_string()),
        (101 << 8, drop.clone(), overflow.to_string()),
        (6, complete.clone(), overflow.to_string()),
        (6, drop.clone(), "thread panicked at unrelated work".into()),
        (6, drop.clone(), format!("{overflow}extra error\n")),
        (0, complete.clone(), "setup warning".into()),
        (0, complete.replace("123-456", "foreign"), String::new()),
        (
            0,
            complete.replace("ready property", "ready wrong"),
            String::new(),
        ),
        (
            0,
            complete.replace("drop property", "drop wrong"),
            String::new(),
        ),
        (
            0,
            complete.replace("values 100000,1", "values bad"),
            String::new(),
        ),
        (
            0,
            complete.replace("values 100000,1", "values 1"),
            String::new(),
        ),
        (
            0,
            complete.replace("values 100000,1", "values 100000,2"),
            String::new(),
        ),
        (
            0,
            complete.replace(&format!("{prefix}drop property\n"), ""),
            String::new(),
        ),
        (
            0,
            format!("{complete}{prefix}complete property\n"),
            String::new(),
        ),
        (
            0,
            format!("{prefix}ready property\n{complete}"),
            String::new(),
        ),
        (0, String::new(), String::new()),
    ] {
        assert!(stack_probe::classify("123-456", "property", 2, status(native), out, err).is_err());
    }
}

#[test]
fn a_native_valid_workload_completes_with_its_observations() {
    let result = stack_probe::observe(
        "a_native_valid_workload_completes_with_its_observations",
        2,
        || (vec![7, 1], vec![pw_render::Value::Int(7)]),
    )
    .unwrap();
    if let Some(result) = result {
        assert!(!result.overflow);
        assert_eq!(result.values, vec![7, 1]);
    }
}

#[test]
fn a_native_setup_panic_is_invalid() {
    let result = stack_probe::observe("a_native_setup_panic_is_invalid", 2, || {
        panic!("unrelated setup failure")
    });
    assert!(result.is_err());
}

#[test]
fn a_native_exit_without_the_workload_receipt_is_invalid() {
    let result = stack_probe::observe(
        "a_native_exit_without_the_workload_receipt_is_invalid",
        2,
        || std::process::exit(0),
    );
    assert!(result.is_err());
}

#[test]
fn a_native_unrelated_abort_is_invalid() {
    let result = stack_probe::observe("a_native_unrelated_abort_is_invalid", 2, || {
        std::process::abort()
    });
    assert!(result.is_err());
}

#[test]
fn a_native_timeout_is_reaped_and_invalid() {
    let result = stack_probe::observe_bounded(
        "a_native_timeout_is_reaped_and_invalid",
        2,
        Duration::from_millis(100),
        || {
            std::thread::sleep(Duration::from_secs(60));
            (vec![1], Vec::new())
        },
    );
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("timeout/wait"));
}
