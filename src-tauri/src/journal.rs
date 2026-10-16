use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, NaiveDate};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedMarkdown {
    pub file_name: String,
    pub title: String,
    pub date: Option<String>,
    pub tags: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNotice {
    pub file_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub entries: Vec<ListedMarkdown>,
    pub notices: Vec<FileNotice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalSnapshot {
    pub folder_name: Option<String>,
    pub folder_path: Option<String>,
    pub accessible: bool,
    pub entries: Vec<ListedMarkdown>,
    pub notices: Vec<FileNotice>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum JournalError {
    Missing { path: PathBuf },
    NotDirectory { path: PathBuf },
    Unreadable { path: PathBuf, detail: String },
}

impl JournalError {
    pub fn user_message(&self) -> String {
        match self {
            Self::Missing { path } => format!(
                "The journal folder is missing or was moved ({}).",
                path.display()
            ),
            Self::NotDirectory { path } => {
                format!("That path is not a folder ({}).", path.display())
            }
            Self::Unreadable { path, detail } => format!(
                "Yoman cannot read the journal folder ({}). {detail}",
                path.display()
            ),
        }
    }
}

pub fn unconfigured_snapshot() -> JournalSnapshot {
    JournalSnapshot {
        folder_name: None,
        folder_path: None,
        accessible: false,
        entries: Vec::new(),
        notices: Vec::new(),
        error: None,
    }
}

pub fn error_snapshot(message: String) -> JournalSnapshot {
    JournalSnapshot {
        folder_name: None,
        folder_path: None,
        accessible: false,
        entries: Vec::new(),
        notices: Vec::new(),
        error: Some(message),
    }
}

pub fn snapshot_for_folder(folder: &Path) -> JournalSnapshot {
    let folder_path = Some(folder.display().to_string());
    let folder_name = Some(folder_display_name(folder));
    match list_markdown_files(folder) {
        Ok(listing) => {
            let canonical = folder
                .canonicalize()
                .unwrap_or_else(|_| folder.to_path_buf());
            JournalSnapshot {
                folder_name: Some(folder_display_name(&canonical)),
                folder_path: Some(canonical.display().to_string()),
                accessible: true,
                entries: listing.entries,
                notices: listing.notices,
                error: None,
            }
        }
        Err(err) => JournalSnapshot {
            folder_name,
            folder_path,
            accessible: false,
            entries: Vec::new(),
            notices: Vec::new(),
            error: Some(format!(
                "{} Choose the folder again. No files were changed.",
                err.user_message()
            )),
        },
    }
}

pub fn list_markdown_files(journal_dir: &Path) -> Result<DirectoryListing, JournalError> {
    if !journal_dir.exists() {
        return Err(JournalError::Missing {
            path: journal_dir.to_path_buf(),
        });
    }

    let root = journal_dir
        .canonicalize()
        .map_err(|err| JournalError::Unreadable {
            path: journal_dir.to_path_buf(),
            detail: err.to_string(),
        })?;

    if !root.is_dir() {
        return Err(JournalError::NotDirectory { path: root });
    }

    let read_dir = fs::read_dir(&root).map_err(|err| JournalError::Unreadable {
        path: root.clone(),
        detail: err.to_string(),
    })?;

    let mut entries = Vec::new();
    let mut notices = Vec::new();

    for item in read_dir {
        let item = match item {
            Ok(item) => item,
            Err(err) => {
                notices.push(FileNotice {
                    file_name: root.display().to_string(),
                    reason: format!("Yoman could not read an item in this folder ({err})."),
                });
                continue;
            }
        };

        let raw_name = item.file_name();
        let file_name = match raw_name.to_str() {
            Some(name) => name.to_string(),
            None => {
                notices.push(FileNotice {
                    file_name: raw_name.to_string_lossy().into_owned(),
                    reason: "This file name is not valid UTF-8, so Yoman did not list it."
                        .to_string(),
                });
                continue;
            }
        };

        let path = item.path();
        let canonical = match path.canonicalize() {
            Ok(path) => path,
            Err(err) => {
                notices.push(FileNotice {
                    file_name,
                    reason: format!(
                        "Yoman could not check whether this item stays inside the journal folder ({err})."
                    ),
                });
                continue;
            }
        };

        if !is_inside_journal(&root, &canonical) {
            notices.push(FileNotice {
                file_name,
                reason: "This item points outside the journal folder, so Yoman did not list it."
                    .to_string(),
            });
            continue;
        }

        let metadata = match fs::metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(err) => {
                notices.push(FileNotice {
                    file_name,
                    reason: format!("Yoman could not read this item ({err})."),
                });
                continue;
            }
        };

        if metadata.is_dir() {
            if is_markdown_name(&file_name) {
                notices.push(FileNotice {
                    file_name,
                    reason: "This is a folder, not a Markdown file. Yoman does not look inside subfolders."
                        .to_string(),
                });
            }
            continue;
        }

        if !metadata.is_file() {
            if is_markdown_name(&file_name) {
                notices.push(FileNotice {
                    file_name,
                    reason: "This Markdown name is not a regular file, so Yoman did not list it."
                        .to_string(),
                });
            }
            continue;
        }

        if is_markdown_name(&file_name) {
            match indexed_entry(&canonical, &file_name, metadata.modified().ok()) {
                Ok(entry) => entries.push(entry),
                Err(notice) => notices.push(notice),
            }
        }
    }

    entries.sort_by(|left, right| {
        right
            .sort_ms
            .cmp(&left.sort_ms)
            .then_with(|| left.entry.file_name.cmp(&right.entry.file_name))
    });
    let entries: Vec<ListedMarkdown> = entries.into_iter().map(|row| row.entry).collect();
    notices.sort_by(|left, right| left.file_name.cmp(&right.file_name));

    Ok(DirectoryListing { entries, notices })
}

struct SortedEntry {
    entry: ListedMarkdown,
    sort_ms: i64,
}

fn indexed_entry(
    path: &Path,
    file_name: &str,
    modified: Option<SystemTime>,
) -> Result<SortedEntry, FileNotice> {
    let bytes = fs::read(path).map_err(|err| FileNotice {
        file_name: file_name.to_string(),
        reason: format!("Yoman could not read this file ({err})."),
    })?;
    let text = String::from_utf8(bytes).map_err(|_| FileNotice {
        file_name: file_name.to_string(),
        reason: "This file is not valid UTF-8 text, so Yoman did not list it as an entry."
            .to_string(),
    })?;
    let parsed = crate::entry::parse_entry(file_name, &text);
    if !parsed.readable {
        return Err(FileNotice {
            file_name: file_name.to_string(),
            reason: parsed.warning.unwrap_or_else(|| {
                "Yoman could not read this file, so it is not listed as an entry.".to_string()
            }),
        });
    }
    let modified_ms = modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    Ok(SortedEntry {
        sort_ms: entry_sort_ms(parsed.date.as_deref()).unwrap_or(modified_ms),
        entry: ListedMarkdown {
            file_name: file_name.to_string(),
            title: parsed.title,
            date: parsed.date,
            tags: parsed.tags,
            text: parsed.body,
        },
    })
}

fn entry_sort_ms(date: Option<&str>) -> Option<i64> {
    let date = date?;
    if let Ok(parsed) = DateTime::parse_from_rfc3339(date) {
        return Some(parsed.timestamp_millis());
    }
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .and_then(|day| day.and_hms_opt(0, 0, 0))
        .map(|time| time.and_utc().timestamp_millis())
}

pub fn is_markdown_name(file_name: &str) -> bool {
    Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

pub fn is_inside_journal(root: &Path, candidate: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return false;
    };
    candidate.starts_with(root)
}

fn folder_display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("yoman-journal-{nanos}-{id}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lists_only_markdown_files_in_the_selected_folder() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(journal.join("nested")).unwrap();
        fs::write(journal.join("Alpha.md"), "alpha").unwrap();
        fs::write(journal.join("notes.MD"), "notes").unwrap();
        fs::write(journal.join("שלום.md"), "שלום").unwrap();
        touch(&journal.join("Alpha.md"), 10);
        touch(&journal.join("notes.MD"), 20);
        touch(&journal.join("שלום.md"), 30);
        fs::write(journal.join("ignore.txt"), "nope").unwrap();
        fs::write(journal.join("nested/hidden.md"), "hidden").unwrap();

        let listing = list_markdown_files(&journal).unwrap();
        let names: Vec<_> = listing
            .entries
            .iter()
            .map(|entry| entry.file_name.as_str())
            .collect();

        assert_eq!(names, vec!["שלום.md", "notes.MD", "Alpha.md"]);
        assert!(listing.notices.is_empty());
    }

    fn touch(path: &Path, secs: u64) {
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .unwrap();
    }

    #[test]
    fn entries_sort_by_frontmatter_date_before_file_time() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(&journal).unwrap();
        fs::write(
            journal.join("newer-file.md"),
            "---\ndate: \"2020-01-01\"\ntitle: \"Old\"\n---\nold\n",
        )
        .unwrap();
        fs::write(
            journal.join("older-file.md"),
            "---\ndate: \"2026-10-16T12:00:00Z\"\ntitle: \"New\"\ntags:\n  - אישי\n---\nשלום\n",
        )
        .unwrap();
        touch(&journal.join("newer-file.md"), 500);
        touch(&journal.join("older-file.md"), 100);

        let listing = list_markdown_files(&journal).unwrap();
        assert_eq!(listing.entries[0].title, "New");
        assert_eq!(listing.entries[0].tags, vec!["אישי".to_string()]);
        assert!(listing.entries[0].text.contains("שלום"));
        assert_eq!(listing.entries[1].title, "Old");
    }

    #[test]
    fn interrupted_save_files_are_not_entries() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(&journal).unwrap();
        fs::write(journal.join("note.md"), "hello\n").unwrap();
        fs::write(journal.join(".note.md.1.2.yoman-tmp"), "partial").unwrap();
        let listing = list_markdown_files(&journal).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].file_name, "note.md");
    }

    #[test]
    fn listing_does_not_change_file_bytes_or_mtime() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(&journal).unwrap();
        let file = journal.join("entry.md");
        let body = "שלום from Cursor\n";
        fs::write(&file, body).unwrap();
        let modified = fs::metadata(&file).unwrap().modified().unwrap();

        let listing = list_markdown_files(&journal).unwrap();

        assert_eq!(listing.entries.len(), 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), body);
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn reports_symlinks_that_leave_the_journal_folder() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&journal).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.md"), "secret").unwrap();
        fs::write(journal.join("inside.md"), "inside").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.md"), journal.join("secret.md")).unwrap();

        let listing = list_markdown_files(&journal).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].file_name, "inside.md");
        assert_eq!(listing.notices.len(), 1);
        assert_eq!(listing.notices[0].file_name, "secret.md");
        assert!(listing.notices[0].reason.contains("outside"));
        assert_eq!(
            fs::read_to_string(outside.join("secret.md")).unwrap(),
            "secret"
        );
    }

    #[test]
    fn reports_non_utf8_file_names() {
        use std::os::unix::ffi::OsStringExt;

        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(&journal).unwrap();
        let name = std::ffi::OsString::from_vec(b"bad-\xFF.md".to_vec());
        fs::write(journal.join(name), "x").unwrap();

        let listing = list_markdown_files(&journal).unwrap();
        assert!(listing.entries.is_empty());
        assert_eq!(listing.notices.len(), 1);
        assert!(listing.notices[0].reason.contains("UTF-8"));
    }

    #[test]
    fn missing_and_non_directory_paths_fail_without_creating_files() {
        let temp = TempDir::new();
        let missing = temp.path().join("missing");
        let err = list_markdown_files(&missing).unwrap_err();
        assert!(matches!(err, JournalError::Missing { .. }));
        assert!(!missing.exists());

        let file = temp.path().join("file.md");
        fs::write(&file, "x").unwrap();
        let err = list_markdown_files(&file).unwrap_err();
        assert!(matches!(err, JournalError::NotDirectory { .. }));
        assert_eq!(fs::read_to_string(&file).unwrap(), "x");
    }

    #[test]
    fn inside_check_rejects_paths_outside_the_root() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(journal.join("nested")).unwrap();
        let inside = journal.join("inside.md");
        fs::write(&inside, "ok").unwrap();
        let outside = temp.path().join("outside.md");
        fs::write(&outside, "no").unwrap();

        assert!(is_inside_journal(
            &journal,
            &journal.join("nested/../inside.md")
        ));
        assert!(!is_inside_journal(&journal, &outside));
        assert!(!is_inside_journal(
            &journal,
            &temp.path().join("journal-evil.md")
        ));
    }

    #[test]
    fn empty_folder_is_a_successful_empty_list() {
        let temp = TempDir::new();
        let journal = temp.path().join("journal");
        fs::create_dir_all(&journal).unwrap();
        let listing = list_markdown_files(&journal).unwrap();
        assert!(listing.entries.is_empty());
        assert!(listing.notices.is_empty());
    }
}
