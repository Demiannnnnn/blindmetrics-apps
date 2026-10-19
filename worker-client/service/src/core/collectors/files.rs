use crate::core::metrics::{FileAction, FileItem};
use chrono::Utc;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Metadata-only corporate file activity monitor (DP-03, RF-AGENT-03-2).
/// Strictly prohibited from opening, indexing, or reading any file contents.
pub struct FileCollector {
    watched_root: PathBuf,
    events: Arc<Mutex<Vec<FileItem>>>,
    _watcher: Option<RecommendedWatcher>,
}

impl FileCollector {
    /// Creates a new FileCollector for a designated corporate directory root.
    pub fn new(watched_root: PathBuf) -> Self {
        Self {
            watched_root,
            events: Arc::new(Mutex::new(Vec::new())),
            _watcher: None,
        }
    }

    /// Starts filesystem monitoring on the designated corporate directory.
    pub fn start_watching(&mut self) -> Result<(), notify::Error> {
        let events_clone = self.events.clone();
        let root_clone = self.watched_root.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    Self::handle_event(&root_clone, &events_clone, event);
                }
                Err(err) => {
                    tracing::warn!("File watcher dropped event or encountered error: {:?}", err);
                }
            },
            notify::Config::default(),
        )?;

        if self.watched_root.exists() {
            watcher.watch(&self.watched_root, RecursiveMode::Recursive)?;
        }

        self._watcher = Some(watcher);
        Ok(())
    }

    /// Handles a notify filesystem event, filtering temporary and internal files.
    pub fn handle_event(root: &Path, events: &Arc<Mutex<Vec<FileItem>>>, event: Event) {
        let action = match event.kind {
            EventKind::Create(_) => FileAction::Created,
            EventKind::Modify(_) => FileAction::Modified,
            EventKind::Remove(_) => FileAction::Deleted,
            EventKind::Any | EventKind::Other | EventKind::Access(_) => return,
        };

        let now = Utc::now();
        let mut new_items = Vec::new();

        for path in event.paths {
            // Ignore system directories, git, and temporary office files
            if is_ignored_path(&path) {
                continue;
            }

            let relative_path = match path.strip_prefix(root) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => path.to_string_lossy().to_string(),
            };

            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();

            new_items.push(FileItem {
                ts: now,
                action,
                watched_root: root.to_string_lossy().to_string(),
                relative_path,
                previous_relative_path: None,
                extension,
            });
        }

        if !new_items.is_empty() {
            let mut lock = match events.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            lock.extend(new_items);
        }
    }

    /// Drains current accumulated file activity metadata records.
    pub fn drain(&self) -> Vec<FileItem> {
        let mut lock = match self.events.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        std::mem::take(&mut *lock)
    }
}

/// Identifies files to ignore: .git repositories, temp files, and system caches (RF-AGENT-03-2).
pub fn is_ignored_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Check for git or version control directories
    if path_str.contains("/.git/")
        || path_str.ends_with("/.git")
        || path_str.contains("/.svn/")
        || path_str.contains("/.hg/")
    {
        return true;
    }

    // Check file/dir name
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        // Hidden files, system files, and lock files
        if file_name.starts_with('.')
            || file_name.starts_with("~$")
            || file_name.starts_with(".~lock.")
            || file_name == "Thumbs.db"
            || file_name == "desktop.ini"
            || file_name == ".DS_Store"
        {
            return true;
        }

        // Temporary file extensions
        if file_name.ends_with(".tmp") || file_name.ends_with(".swp") || file_name.ends_with(".bak")
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ignored_path() {
        assert!(is_ignored_path(Path::new("/corp/work/.git/config")));
        assert!(is_ignored_path(Path::new("/corp/work/doc.tmp")));
        assert!(is_ignored_path(Path::new("/corp/work/~$proposal.docx")));
        assert!(is_ignored_path(Path::new("/corp/work/.DS_Store")));
        assert!(is_ignored_path(Path::new("/corp/work/Thumbs.db")));

        assert!(!is_ignored_path(Path::new("/corp/work/contract.pdf")));
        assert!(!is_ignored_path(Path::new("/corp/work/src/main.rs")));
    }

    #[test]
    fn test_file_collector_records_metadata_without_content_read() {
        let root = PathBuf::from("/Users/employee/Documents/Corporate");
        let collector = FileCollector::new(root.clone());

        let fake_event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![
                root.join("report.docx"),
                root.join(".git/index"),  // Should be ignored
                root.join("~$temp.docx"), // Should be ignored
            ],
            attrs: Default::default(),
        };

        FileCollector::handle_event(&root, &collector.events, fake_event);

        let items = collector.drain();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].relative_path, "report.docx");
        assert_eq!(items[0].extension, "docx");
        assert_eq!(items[0].action, FileAction::Created);
        assert_eq!(items[0].watched_root, root.to_string_lossy());

        // Drain resets buffer
        assert!(collector.drain().is_empty());
    }
}
