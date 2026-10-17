use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn mixed_workers_wait_for_memory_even_when_a_process_slot_is_free() {
    let budget = Budget::new(2048, 4);
    let first = budget.acquire(1024, &|| Ok(()), Duration::ZERO).unwrap();
    let second = budget.acquire(512, &|| Ok(()), Duration::ZERO).unwrap();
    assert_eq!(
        budget
            .acquire(1024, &|| Ok(()), Duration::ZERO)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(second);
    let third = budget.acquire(1024, &|| Ok(()), Duration::ZERO).unwrap();
    assert_eq!(budget.usage.lock().unwrap().bytes, 2048);
    drop((first, third));
    assert_eq!(budget.usage.lock().unwrap().bytes, 0);
}

#[test]
fn large_workers_leave_an_envelope_and_process_slot_for_navigation() {
    let budget = Budget::with_reserve(2560, 4, 512);
    let bulk = budget.acquire(1024, &|| Ok(()), Duration::ZERO).unwrap();
    let targeted = budget.acquire(1024, &|| Ok(()), Duration::ZERO).unwrap();
    assert!(budget.acquire(1024, &|| Ok(()), Duration::ZERO).is_err());
    let navigation = budget.acquire(512, &|| Ok(()), Duration::ZERO).unwrap();
    assert_eq!(budget.usage.lock().unwrap().bytes, 2560);
    drop((bulk, targeted, navigation));
    let budget = Budget::with_reserve(4096, 4, 512);
    let jobs = (0..3)
        .map(|_| budget.acquire(600, &|| Ok(()), Duration::ZERO).unwrap())
        .collect::<Vec<_>>();
    assert!(budget.acquire(600, &|| Ok(()), Duration::ZERO).is_err());
    assert!(budget.acquire(512, &|| Ok(()), Duration::ZERO).is_ok());
    drop(jobs);
}

#[test]
fn seven_encoder_reservation_preserves_navigation_without_admitting_a_second_bulk_job() {
    let budget = Budget::with_reserve(MEMORY_BYTES, MAX_WORKERS, INTERACTIVE_RESERVE);
    let encoder = budget
        .acquire(2 * 1024 * 1024 * 1024, &|| Ok(()), Duration::ZERO)
        .unwrap();
    assert!(budget
        .acquire(1024 * 1024 * 1024, &|| Ok(()), Duration::ZERO)
        .is_err());
    let navigation = budget
        .acquire(INTERACTIVE_RESERVE, &|| Ok(()), Duration::ZERO)
        .unwrap();
    assert_eq!(budget.usage.lock().unwrap().bytes, MEMORY_BYTES);
    drop((encoder, navigation));
    assert_eq!(budget.usage.lock().unwrap().bytes, 0);
}

#[test]
fn cancelled_wait_and_failed_preparation_return_to_a_usable_budget() {
    let budget = Budget::new(1024, 1);
    let permit = budget.acquire(512, &|| Ok(()), Duration::ZERO).unwrap();
    let calls = AtomicUsize::new(0);
    let error = budget
        .acquire(
            512,
            &|| {
                if calls.fetch_add(1, Ordering::SeqCst) > 1 {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
                } else {
                    Ok(())
                }
            },
            Duration::from_secs(1),
        )
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    drop(permit);
    let result = (|| -> io::Result<()> {
        let _permit = budget.acquire(1024, &|| Ok(()), Duration::ZERO)?;
        Err(io::Error::other("preparation failed"))
    })();
    assert!(result.is_err());
    assert!(budget.acquire(1024, &|| Ok(()), Duration::ZERO).is_ok());
}

#[test]
fn concurrent_mixed_envelopes_never_exceed_the_common_limits() {
    let budget = Budget::new(2048, 4);
    let peak = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|scope| {
        for index in 0..24 {
            let budget = budget.clone();
            let peak = peak.clone();
            scope.spawn(move || {
                let permit = budget
                    .acquire(
                        if index % 3 == 0 { 1024 } else { 512 },
                        &|| Ok(()),
                        Duration::from_secs(5),
                    )
                    .unwrap();
                {
                    let usage = budget.usage.lock().unwrap();
                    assert!(usage.bytes <= 2048 && usage.workers <= 4);
                    peak.fetch_max(usage.bytes, Ordering::SeqCst);
                }
                std::thread::sleep(Duration::from_millis(5));
                drop(permit);
            });
        }
    });
    let usage = budget.usage.lock().unwrap();
    assert_eq!((usage.bytes, usage.workers), (0, 0));
    assert!(peak.load(Ordering::SeqCst) >= 1024);
}

#[test]
fn shared_job_has_native_aggregate_limits_without_breakaway() {
    let job = create_job().unwrap();
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    unsafe {
        QueryInformationJobObject(
            Some(HANDLE(job.as_raw_handle())),
            JobObjectExtendedLimitInformation,
            (&mut info as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
            None,
        )
    }
    .unwrap();
    assert_eq!(info.JobMemoryLimit, MEMORY_BYTES);
    assert_eq!(
        info.BasicLimitInformation.ActiveProcessLimit,
        MAX_WORKERS as u32
    );
    assert_eq!(
        (info.BasicLimitInformation.LimitFlags
            & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK))
            .0,
        0
    );
    let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
    unsafe {
        QueryInformationJobObject(
            Some(HANDLE(job.as_raw_handle())),
            JobObjectCpuRateControlInformation,
            (&mut cpu as *mut JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
            std::mem::size_of_val(&cpu) as u32,
            None,
        )
    }
    .unwrap();
    assert_eq!(unsafe { cpu.Anonymous.CpuRate }, 7000);
    assert_eq!(
        cpu.ControlFlags,
        JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP
    );
}

#[test]
fn native_aggregate_job_refuses_combined_allocations_that_fit_each_individual_job() {
    use std::io::{BufRead, Write};
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    let root = create_job_with_limits(64 * 1024 * 1024, 4).unwrap();
    let mut children = Vec::new();
    let mut jobs = Vec::new();
    for expected in [true, false] {
        let job = super::super::windows_preview_worker::job(128 * 1024 * 1024).unwrap();
        let mut command =
            super::super::windows_process_launch::native_probe::command("memory-bytes");
        command
            .arg((48 * 1024 * 1024).to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x08000000);
        let mut child = super::super::windows_process_launch::spawn(command).unwrap();
        unsafe {
            AssignProcessToJobObject(HANDLE(root.as_raw_handle()), HANDLE(child.as_raw_handle()))
                .unwrap();
            AssignProcessToJobObject(HANDLE(job.as_raw_handle()), HANDLE(child.as_raw_handle()))
                .unwrap();
        }
        child.stdin.as_mut().unwrap().write_all(&[0xA7]).unwrap();
        let mut report = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut report)
            .unwrap();
        children.push(child);
        jobs.push(job);
        assert_eq!(report.trim(), format!("allocation={expected}"));
    }
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(jobs);
}

#[test]
fn delayed_exit_retains_admission_until_native_exit_is_confirmed() {
    use std::os::windows::process::CommandExt;
    let budget = Budget::new(1024, 1);
    let permit = budget.acquire(512, &|| Ok(()), Duration::ZERO).unwrap();
    let mut command = super::super::windows_process_launch::native_probe::command("hold-no-gate");
    command.creation_flags(0x08000000);
    let mut child = super::super::windows_process_launch::spawn(command).unwrap();
    let cleaned = Arc::new(AtomicUsize::new(0));
    let cleanup_witness = cleaned.clone();
    retain_until_exit(HANDLE(child.as_raw_handle()), permit, move || {
        cleanup_witness.store(1, Ordering::SeqCst);
    });
    assert!(budget.acquire(512, &|| Ok(()), Duration::ZERO).is_err());
    assert_eq!(cleaned.load(Ordering::SeqCst), 0);
    child.kill().unwrap();
    child.wait().unwrap();
    let replacement = budget
        .acquire(1024, &|| Ok(()), Duration::from_secs(3))
        .unwrap();
    drop(replacement);
    assert_eq!(cleaned.load(Ordering::SeqCst), 1);
    assert_eq!(budget.usage.lock().unwrap().workers, 0);
}
