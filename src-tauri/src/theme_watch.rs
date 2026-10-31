//! Nonrecursive directory-entry watches, including resolved symlink target ancestors.
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

enum Message {
    Change(notify::Result<Event>),
    Stop,
}
pub struct ThemeWatch {
    stop: mpsc::Sender<Message>,
    thread: Option<thread::JoinHandle<()>>,
}
type Entries = BTreeMap<PathBuf, BTreeSet<std::ffi::OsString>>;
fn chain(file: &std::path::Path, entries: &mut Entries) {
    let mut child = file;
    while let Some(parent) = child.parent() {
        if let (Ok(real), Some(name)) = (parent.canonicalize(), child.file_name()) {
            if real.is_dir() {
                entries.entry(real).or_default().insert(name.to_owned());
            }
        }
        child = parent;
    }
}
fn entries(files: &[PathBuf]) -> Entries {
    let mut result = Entries::new();
    for file in files {
        chain(file, &mut result);
        for prefix in file.ancestors() {
            if let Ok(real) = prefix.canonicalize() {
                chain(&real, &mut result);
            }
        }
    }
    result
}
fn relevant(event: &notify::Result<Event>, entries: &Entries) -> bool {
    match event {
        Err(_) => true,
        Ok(e) if matches!(e.kind, EventKind::Access(_)) => false,
        Ok(e) => {
            e.paths.is_empty()
                || e.paths.iter().any(|p| {
                    entries.contains_key(p)
                        || p.parent()
                            .and_then(|d| entries.get(d))
                            .is_some_and(|names| p.file_name().is_some_and(|n| names.contains(n)))
                })
        }
    }
}
impl ThemeWatch {
    pub fn new(files: Vec<PathBuf>, changed: impl Fn() + Send + 'static) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let event_tx = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event| {
                let _ = event_tx.send(Message::Change(event));
            },
            notify::Config::default(),
        )?;
        let mut watched = entries(&files);
        for dir in watched.keys() {
            let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
        }
        let thread = thread::spawn(move || {
            while let Ok(message) = rx.recv() {
                match message {
                    Message::Stop => break,
                    Message::Change(e) if !relevant(&e, &watched) => continue,
                    Message::Change(_) => {}
                }
                // This timeout exists only while an actual change is pending.
                let mut deadline = Instant::now() + Duration::from_millis(50);
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(Message::Stop) => return,
                        Ok(Message::Change(event)) if relevant(&event, &watched) => {
                            deadline = Instant::now() + Duration::from_millis(50);
                        }
                        Ok(Message::Change(_)) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                    }
                }
                let next = entries(&files);
                for dir in watched.keys().filter(|d| !next.contains_key(*d)) {
                    let _ = watcher.unwatch(dir);
                }
                // Reattempt failed watches on another relevant event as well.
                for dir in next.keys() {
                    let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
                }
                watched = next;
                changed();
            }
        });
        Ok(Self {
            stop: tx,
            thread: Some(thread),
        })
    }
}
impl Drop for ThemeWatch {
    fn drop(&mut self) {
        let _ = self.stop.send(Message::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
