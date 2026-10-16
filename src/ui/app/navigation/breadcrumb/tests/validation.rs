use super::*;
use std::prelude::v1::test;

#[test]
fn delayed_validation_does_not_block_the_executor_and_old_owners_are_rejected() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let validation = tokio::spawn(validate_path_in_background("slow".into(), move |_| {
            started.send(()).unwrap();
            wait.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            false
        }));
        ready.await.unwrap();
        let owner = Owner {
            revision: 1,
            pane: "pane".into(),
            tab: "tab".into(),
            location: "old".into(),
            text: "slow".into(),
        };
        for change in 0..5 {
            let mut current = owner.clone();
            match change {
                0 => current.revision += 1,
                1 => current.pane.push('x'),
                2 => current.tab.push('x'),
                3 => current.location.push('x'),
                _ => current.text.push('x'),
            }
            assert_ne!(current, owner);
        }
        // The current-thread executor keeps servicing timers while is_dir is blocked.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(!validation.is_finished());
        release.send(()).unwrap();
        assert!(!validation.await.unwrap());
        assert!(
            validate_path_in_background("__network_vincinity__".into(), |_| panic!(
                "virtual root must not access disk"
            ))
            .await
        );
        if cfg!(target_os = "windows") {
            assert!(
                validate_path_in_background(r"\\server".into(), |_| panic!(
                    "UNC server must not access disk"
                ))
                .await
            );
        }
    });
}
