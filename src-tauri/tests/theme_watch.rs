use omawser::theme_watch::ThemeWatch;
use std::{fs, sync::mpsc, time::Duration};

#[test]
fn follows_missing_paths_atomic_replacement_and_symlink_switch() {
    let root = std::env::temp_dir().join(format!("omawser-watch-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let primary = root.join("state/omarchy/current/theme/colors.toml");
    let fallback = root.join("config/omarchy/current/theme/colors.toml");
    let (tx, rx) = mpsc::channel();
    let watch = ThemeWatch::new(vec![primary.clone(), fallback.clone()], move || {
        let _ = tx.send(());
    })
    .unwrap();
    let changed = || {
        rx.recv_timeout(Duration::from_secs(3))
            .expect("watch event")
    };
    fs::create_dir_all(primary.parent().unwrap()).unwrap();
    fs::write(&primary, "accent = '#123456'").unwrap();
    changed();
    // Wait for coalesced events before each distinct transition.
    while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
    let next = primary.with_extension("next");
    fs::write(&next, "accent = '#abcdef'").unwrap();
    fs::rename(next, &primary).unwrap();
    changed();
    while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
    let linked = root.join("linked");
    fs::create_dir(&linked).unwrap();
    fs::write(linked.join("colors.toml"), "accent = '#998877'").unwrap();
    let temp_link = primary.parent().unwrap().with_extension("next");
    std::os::unix::fs::symlink(&linked, &temp_link).unwrap();
    fs::rename(primary.parent().unwrap(), root.join("old-theme")).unwrap();
    fs::rename(&temp_link, primary.parent().unwrap()).unwrap();
    changed();
    while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
    fs::write(linked.join("colors.toml"), "accent = '#112233'").unwrap();
    changed();
    drop(watch);
    while rx.try_recv().is_ok() {}
    fs::write(linked.join("colors.toml"), "accent = '#ffffff'").unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrelated_directory_activity_does_not_starve_pending_refresh() {
    let root = std::env::temp_dir().join(format!("omawser-watch-noise-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let colors = root.join("colors.toml");
    fs::write(&colors, "accent = '#123456'").unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = ThemeWatch::new(vec![colors.clone()], move || {
        let _ = tx.send(());
    })
    .unwrap();
    fs::write(&colors, "accent = '#abcdef'").unwrap();
    let noisy_root = root.clone();
    let noise = std::thread::spawn(move || {
        for _ in 0..100 {
            fs::write(noisy_root.join("unrelated"), "noise").unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let result = rx.recv_timeout(Duration::from_millis(300));
    noise.join().unwrap();
    drop(watcher);
    fs::remove_dir_all(root).unwrap();
    result.expect("unrelated writes must not extend theme debounce");
}
