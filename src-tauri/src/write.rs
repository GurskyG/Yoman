use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::entry::{self, OpenedEntry};
use crate::journal::{self, JournalError, JournalSnapshot};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponse {
    pub saved: bool,
    pub conflict: bool,
    pub entry: Option<OpenedEntry>,
    pub disk_original: Option<String>,
    pub journal: Option<JournalSnapshot>,
    pub message: Option<String>,
}

#[derive(Debug)]
pub enum WriteError {
    InvalidName,
    Outside,
    MissingFolder(JournalError),
    Unreadable { detail: String },
    Exists,
    MalformedFrontmatter,
    Io { detail: String },
}

impl WriteError {
    pub fn user_message(&self) -> String {
        match self {
            Self::InvalidName | Self::Outside => {
                "Yoman only writes a Markdown file directly inside the selected journal folder. Nothing was overwritten.".to_string()
            }
            Self::MissingFolder(err) => format!("{} No file was written.", err.user_message()),
            Self::Unreadable { detail } => format!(
                "Yoman could not read the file before saving ({detail}). Your text is still in the editor."
            ),
            Self::Exists => {
                "That file name already exists, so Yoman did not overwrite it.".to_string()
            }
            Self::MalformedFrontmatter => {
                "The frontmatter could not be read, so Yoman did not write the file. Your text is still in the editor.".to_string()
            }
            Self::Io { detail } => format!(
                "Yoman could not save the file ({detail}). Your text is still in the editor."
            ),
        }
    }
}

pub fn create_entry(
    journal_dir: &Path,
    now: DateTime<Utc>,
    date: &str,
    stamp: &str,
) -> Result<OpenedEntry, WriteError> {
    let title = "Untitled";
    let id = new_id(now);
    let mut existing = markdown_names(journal_dir)?;
    let mut file_name = allocate_filename(&existing, date, title, &id);
    let markdown = render_new_entry(&id, title, stamp);
    for _ in 0..20 {
        match write_file(journal_dir, &file_name, &markdown, WriteMode::Create)? {
            WriteOutcome::Saved => {
                return entry::read_entry(journal_dir, &file_name).map_err(map_entry_error);
            }
            WriteOutcome::Exists => {
                existing.insert(file_name);
                file_name = allocate_filename(&existing, date, title, &new_id(now));
            }
            WriteOutcome::Conflict { .. } => {
                return Err(WriteError::Exists);
            }
        }
    }
    Err(WriteError::Exists)
}

pub fn current_clock() -> (DateTime<Utc>, String, String) {
    let local = chrono::Local::now();
    let date = local.format("%Y-%m-%d-%H%M%S").to_string();
    let stamp = local.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    (local.with_timezone(&Utc), date, stamp)
}

pub fn save_entry(
    journal_dir: &Path,
    file_name: &str,
    title: &str,
    body: &str,
    base_original: &str,
    updated: &str,
    tags: &[String],
) -> Result<SaveOutcome, WriteError> {
    let markdown = compose_markdown(base_original, title, body, updated, tags)?;
    place_updated_file(
        journal_dir,
        file_name,
        title,
        &markdown,
        Some(base_original),
        &filename_stamp_from_updated(updated),
    )
}

pub fn overwrite_entry(
    journal_dir: &Path,
    file_name: &str,
    title: &str,
    body: &str,
    base_original: &str,
    updated: &str,
    tags: &[String],
) -> Result<OpenedEntry, WriteError> {
    let markdown = compose_markdown(base_original, title, body, updated, tags)?;
    match place_updated_file(
        journal_dir,
        file_name,
        title,
        &markdown,
        None,
        &filename_stamp_from_updated(updated),
    )? {
        SaveOutcome::Saved { entry } => Ok(entry),
        SaveOutcome::Conflict { .. } => Err(WriteError::Io {
            detail: "the replace did not finish".to_string(),
        }),
    }
}

pub fn save_copy(
    journal_dir: &Path,
    title: &str,
    body: &str,
    base_original: &str,
    now: DateTime<Utc>,
    date: &str,
    updated: &str,
    tags: &[String],
) -> Result<OpenedEntry, WriteError> {
    let markdown = compose_markdown(base_original, title, body, updated, tags)?;
    let mut existing = markdown_names(journal_dir)?;
    let mut file_name = allocate_filename(&existing, date, title, &new_id(now));
    for _ in 0..20 {
        match write_file(journal_dir, &file_name, &markdown, WriteMode::Create)? {
            WriteOutcome::Saved => {
                return entry::read_entry(journal_dir, &file_name).map_err(map_entry_error);
            }
            WriteOutcome::Exists => {
                existing.insert(file_name);
                file_name = allocate_filename(&existing, date, title, &new_id(now));
            }
            WriteOutcome::Conflict { .. } => return Err(WriteError::Exists),
        }
    }
    Err(WriteError::Exists)
}

#[derive(Debug)]
pub enum SaveOutcome {
    Saved { entry: OpenedEntry },
    Conflict { disk: String },
}

enum WriteMode<'a> {
    Create,
    Update { expected: &'a str },
    Overwrite,
}

enum WriteOutcome {
    Saved,
    Exists,
    Conflict { disk: String },
}

fn place_updated_file(
    journal_dir: &Path,
    file_name: &str,
    title: &str,
    markdown: &str,
    expected: Option<&str>,
    fallback_stamp: &str,
) -> Result<SaveOutcome, WriteError> {
    if !is_direct_markdown_name(file_name) {
        return Err(WriteError::InvalidName);
    }
    let existing = markdown_names(journal_dir)?;
    let destination = target_filename(file_name, title, &existing, fallback_stamp);
    if destination == file_name {
        let mode = match expected {
            Some(expected) => WriteMode::Update { expected },
            None => WriteMode::Overwrite,
        };
        return match write_file(journal_dir, file_name, markdown, mode)? {
            WriteOutcome::Saved => Ok(SaveOutcome::Saved {
                entry: entry::read_entry(journal_dir, file_name).map_err(map_entry_error)?,
            }),
            WriteOutcome::Conflict { disk } => Ok(SaveOutcome::Conflict { disk }),
            WriteOutcome::Exists => Err(WriteError::MissingFolder(JournalError::Missing {
                path: journal_dir.join(file_name),
            })),
        };
    }

    if let Some(expected) = expected {
        match fs::read_to_string(journal_root(journal_dir)?.join(file_name)) {
            Ok(disk) if disk != expected => return Ok(SaveOutcome::Conflict { disk }),
            Ok(_) => {}
            Err(err) => {
                return Err(WriteError::Unreadable {
                    detail: err.to_string(),
                });
            }
        }
    }

    let mut reserved = existing;
    reserved.remove(file_name);
    let prefix = filename_datetime_prefix(file_name).unwrap_or(fallback_stamp);
    let mut dest = destination;
    for _ in 0..20 {
        match write_file(journal_dir, &dest, markdown, WriteMode::Create)? {
            WriteOutcome::Saved => {
                let old_path = journal_root(journal_dir)?.join(file_name);
                if let Err(err) = fs::remove_file(&old_path) {
                    let _ = fs::remove_file(journal_root(journal_dir)?.join(&dest));
                    return Err(WriteError::Io {
                        detail: format!(
                            "the renamed file could not replace the previous one ({err})"
                        ),
                    });
                }
                return Ok(SaveOutcome::Saved {
                    entry: entry::read_entry(journal_dir, &dest).map_err(map_entry_error)?,
                });
            }
            WriteOutcome::Exists => {
                reserved.insert(dest);
                dest = allocate_filename(&reserved, prefix, title, &new_id(Utc::now()));
            }
            WriteOutcome::Conflict { .. } => return Err(WriteError::Exists),
        }
    }
    Err(WriteError::Exists)
}

pub fn delete_entry(journal_dir: &Path, file_name: &str) -> Result<(), WriteError> {
    if !is_direct_markdown_name(file_name) {
        return Err(WriteError::InvalidName);
    }
    let root = journal_root(journal_dir)?;
    let target = root.join(file_name);
    let metadata = fs::symlink_metadata(&target).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            WriteError::Unreadable {
                detail: "the file is no longer in the journal folder".to_string(),
            }
        } else {
            WriteError::Unreadable {
                detail: err.to_string(),
            }
        }
    })?;
    if metadata.file_type().is_symlink() {
        let canonical = target
            .canonicalize()
            .map_err(|err| WriteError::Unreadable {
                detail: err.to_string(),
            })?;
        if !journal::is_inside_journal(&root, &canonical) || !canonical.is_file() {
            return Err(WriteError::Outside);
        }
    } else if metadata.is_dir() || !metadata.is_file() {
        return Err(WriteError::InvalidName);
    }
    trash::delete(&target).map_err(|err| WriteError::Io {
        detail: err.to_string(),
    })
}

pub fn target_filename(
    current: &str,
    title: &str,
    existing: &HashSet<String>,
    fallback_stamp: &str,
) -> String {
    let slug = slugify(title);
    let prefix = filename_datetime_prefix(current).unwrap_or(fallback_stamp);
    if filename_datetime_prefix(current).is_some() && same_title_filename(current, prefix, &slug) {
        return current.to_string();
    }
    let mut reserved = existing.clone();
    reserved.remove(current);
    let ideal = format!("{prefix}-{slug}.md");
    if !reserved.contains(&ideal) {
        return ideal;
    }
    allocate_filename(&reserved, prefix, title, fallback_stamp)
}

fn filename_datetime_prefix(file_name: &str) -> Option<&str> {
    let stem = markdown_stem(file_name)?;
    let bytes = stem.as_bytes();
    if bytes.len() >= 18 && bytes[17] == b'-' && is_compact_datetime(&stem[..17]) {
        Some(&stem[..17])
    } else {
        None
    }
}

fn is_compact_datetime(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 17
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && bytes[11..17].iter().all(u8::is_ascii_digit)
}

fn same_title_filename(file_name: &str, prefix: &str, slug: &str) -> bool {
    let Some(stem) = markdown_stem(file_name) else {
        return false;
    };
    let Some(rest) = stem.strip_prefix(&format!("{prefix}-{slug}")) else {
        return false;
    };
    rest.is_empty() || is_collision_tail(rest)
}

fn is_collision_tail(rest: &str) -> bool {
    let Some(rest) = rest.strip_prefix('-') else {
        return false;
    };
    let (head, tail) = match rest.split_once('-') {
        Some((head, tail)) => (head, Some(tail)),
        None => (rest, None),
    };
    head.len() == 4
        && head.chars().all(|ch| ch.is_ascii_hexdigit())
        && tail
            .is_none_or(|number| !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()))
}

fn markdown_stem(file_name: &str) -> Option<&str> {
    if file_name.len() > 3 && file_name[file_name.len() - 3..].eq_ignore_ascii_case(".md") {
        Some(&file_name[..file_name.len() - 3])
    } else {
        None
    }
}

fn filename_stamp_from_updated(updated: &str) -> String {
    DateTime::parse_from_rfc3339(updated)
        .map(|parsed| parsed.format("%Y-%m-%d-%H%M%S").to_string())
        .unwrap_or_else(|_| "untitled".to_string())
}

pub fn allocate_filename(existing: &HashSet<String>, date: &str, title: &str, id: &str) -> String {
    let slug = slugify(title);
    let base = format!("{date}-{slug}.md");
    if !existing.contains(&base) {
        return base;
    }
    let suffix: String = id
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    for n in 0..1000 {
        let name = if n == 0 {
            format!("{date}-{slug}-{suffix}.md")
        } else {
            format!("{date}-{slug}-{suffix}-{n}.md")
        };
        if !existing.contains(&name) {
            return name;
        }
    }
    format!("{date}-{slug}-{id}.md")
}

pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
        if slug.chars().count() >= 48 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

pub fn compose_markdown(
    original: &str,
    title: &str,
    body: &str,
    updated: &str,
    tags: &[String],
) -> Result<String, WriteError> {
    let scan = original.strip_prefix('\u{feff}').unwrap_or(original);
    let (mut map, had_frontmatter) = match split_yaml(scan) {
        YamlSplit::Absent => (Mapping::new(), false),
        YamlSplit::Unclosed => return Err(WriteError::MalformedFrontmatter),
        YamlSplit::Closed { yaml } => {
            if yaml.trim().is_empty() {
                (Mapping::new(), true)
            } else {
                let value: Value =
                    serde_yaml::from_str(yaml).map_err(|_| WriteError::MalformedFrontmatter)?;
                let Some(mapping) = value.as_mapping().cloned() else {
                    return Err(WriteError::MalformedFrontmatter);
                };
                (mapping, true)
            }
        }
    };
    if !had_frontmatter || !has_key(&map, "id") {
        insert_string(&mut map, "id", &new_id(Utc::now()));
    }
    insert_string(&mut map, "title", title.trim());
    if !has_key(&map, "created") && !has_key(&map, "date") {
        insert_string(&mut map, "created", updated);
    }
    insert_string(&mut map, "updated", updated);
    replace_tags(&mut map, tags);
    render_document(&map, body)
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty()
            || normalized
                .iter()
                .any(|existing: &String| existing == trimmed)
        {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn replace_tags(map: &mut Mapping, tags: &[String]) {
    let key = Value::String("tags".to_string());
    let normalized = normalize_tags(tags);
    if normalized.is_empty() {
        map.remove(&key);
        return;
    }
    map.insert(
        key,
        Value::Sequence(normalized.into_iter().map(Value::String).collect()),
    );
}

fn render_new_entry(id: &str, title: &str, stamp: &str) -> String {
    let mut map = Mapping::new();
    insert_string(&mut map, "id", id);
    insert_string(&mut map, "title", title);
    insert_string(&mut map, "created", stamp);
    insert_string(&mut map, "updated", stamp);
    render_document(&map, "").unwrap_or_else(|_| format!("---\nid: \"{id}\"\ntitle: \"{title}\"\ncreated: \"{stamp}\"\nupdated: \"{stamp}\"\n---\n"))
}

fn render_document(map: &Mapping, body: &str) -> Result<String, WriteError> {
    let yaml = serde_yaml::to_string(map).map_err(|err| WriteError::Io {
        detail: err.to_string(),
    })?;
    let yaml = yaml.strip_prefix("---\n").unwrap_or(&yaml);
    let mut body = body.to_string();
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    if body.starts_with('\n') {
        body.remove(0);
    }
    Ok(format!("---\n{yaml}---\n{body}"))
}

enum YamlSplit<'a> {
    Absent,
    Unclosed,
    Closed { yaml: &'a str },
}

fn split_yaml(text: &str) -> YamlSplit<'_> {
    let Some(rest) = text.strip_prefix("---") else {
        return YamlSplit::Absent;
    };
    let Some(rest) = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
    else {
        return YamlSplit::Unclosed;
    };
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content == "---" {
            return YamlSplit::Closed {
                yaml: &rest[..offset],
            };
        }
        offset += line.len();
    }
    YamlSplit::Unclosed
}

fn has_key(map: &Mapping, key: &str) -> bool {
    map.contains_key(Value::String(key.to_string()))
}

fn insert_string(map: &mut Mapping, key: &str, value: &str) {
    map.insert(
        Value::String(key.to_string()),
        Value::String(value.to_string()),
    );
}

fn write_file(
    journal_dir: &Path,
    file_name: &str,
    contents: &str,
    mode: WriteMode<'_>,
) -> Result<WriteOutcome, WriteError> {
    let root = journal_root(journal_dir)?;
    if !is_direct_markdown_name(file_name) {
        return Err(WriteError::InvalidName);
    }
    let target = root.join(file_name);
    if let Some(parent) = target.parent() {
        if parent != root {
            return Err(WriteError::Outside);
        }
    }
    let current = if target.exists() {
        match fs::read(&target) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => Some(text),
                Err(_) => {
                    return Err(WriteError::Unreadable {
                        detail: "the file is not valid UTF-8".to_string(),
                    });
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => None,
            Err(err) => {
                return Err(WriteError::Unreadable {
                    detail: err.to_string(),
                });
            }
        }
    } else {
        None
    };

    match (&mode, current.as_deref()) {
        (WriteMode::Create, Some(_)) => return Ok(WriteOutcome::Exists),
        (WriteMode::Update { expected }, Some(disk)) if disk != *expected => {
            return Ok(WriteOutcome::Conflict {
                disk: disk.to_string(),
            });
        }
        (WriteMode::Update { .. }, None) => {
            return Err(WriteError::Unreadable {
                detail: "the file is no longer in the journal folder".to_string(),
            });
        }
        (WriteMode::Update { expected }, Some(disk)) if disk == *expected && disk == contents => {
            return Ok(WriteOutcome::Saved);
        }
        _ => {}
    }

    let temporary = root.join(temporary_name(file_name));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|err| WriteError::Io {
                detail: err.to_string(),
            })?;
        file.write_all(contents.as_bytes())
            .map_err(|err| WriteError::Io {
                detail: err.to_string(),
            })?;
        file.sync_all().map_err(|err| WriteError::Io {
            detail: err.to_string(),
        })?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }

    if let WriteMode::Update { expected } = &mode {
        match fs::read_to_string(&target) {
            Ok(disk) if disk != *expected => {
                let _ = fs::remove_file(&temporary);
                return Ok(WriteOutcome::Conflict { disk });
            }
            Ok(_) => {}
            Err(err) => {
                let _ = fs::remove_file(&temporary);
                return Err(WriteError::Unreadable {
                    detail: err.to_string(),
                });
            }
        }
    }

    let renamed = match mode {
        WriteMode::Create => rename_without_replace(&temporary, &target),
        WriteMode::Update { .. } | WriteMode::Overwrite => {
            fs::rename(&temporary, &target).map_err(|err| WriteError::Io {
                detail: err.to_string(),
            })
        }
    };
    if let Err(err) = renamed {
        let _ = fs::remove_file(&temporary);
        return match err {
            WriteError::Exists => Ok(WriteOutcome::Exists),
            other => Err(other),
        };
    }
    if let Ok(directory) = File::open(&root) {
        let _ = directory.sync_all();
    }
    Ok(WriteOutcome::Saved)
}

fn rename_without_replace(from: &Path, to: &Path) -> Result<(), WriteError> {
    let from_bytes = from.as_os_str().as_encoded_bytes();
    let to_bytes = to.as_os_str().as_encoded_bytes();
    let from_c = std::ffi::CString::new(from_bytes).map_err(|_| WriteError::InvalidName)?;
    let to_c = std::ffi::CString::new(to_bytes).map_err(|_| WriteError::InvalidName)?;
    const RENAME_NOREPLACE: u32 = 1;
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from_c.as_ptr(),
            libc::AT_FDCWD,
            to_c.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.kind() == io::ErrorKind::AlreadyExists || err.raw_os_error() == Some(libc::EEXIST) {
        return Err(WriteError::Exists);
    }
    Err(WriteError::Io {
        detail: err.to_string(),
    })
}

fn temporary_name(file_name: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(".{file_name}.{}.{n}.yoman-tmp", std::process::id())
}

fn journal_root(journal_dir: &Path) -> Result<PathBuf, WriteError> {
    if !journal_dir.exists() {
        return Err(WriteError::MissingFolder(JournalError::Missing {
            path: journal_dir.to_path_buf(),
        }));
    }
    let root = journal_dir
        .canonicalize()
        .map_err(|err| WriteError::Unreadable {
            detail: err.to_string(),
        })?;
    if !root.is_dir() {
        return Err(WriteError::MissingFolder(JournalError::NotDirectory {
            path: root,
        }));
    }
    Ok(root)
}

fn markdown_names(journal_dir: &Path) -> Result<HashSet<String>, WriteError> {
    let listing = journal::list_markdown_files(journal_dir).map_err(WriteError::MissingFolder)?;
    Ok(listing
        .entries
        .into_iter()
        .map(|entry| entry.file_name)
        .collect())
}

fn is_direct_markdown_name(file_name: &str) -> bool {
    journal::is_markdown_name(file_name)
        && !file_name.contains('/')
        && !file_name.contains('\\')
        && !file_name.contains('\0')
        && Path::new(file_name).components().count() == 1
}

fn new_id(now: DateTime<Utc>) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", now.timestamp_micros(), n)
}

fn map_entry_error(err: entry::EntryError) -> WriteError {
    WriteError::Io {
        detail: err.user_message(),
    }
}

pub fn response_saved(entry: OpenedEntry, journal: Option<JournalSnapshot>) -> SaveResponse {
    SaveResponse {
        saved: true,
        conflict: false,
        entry: Some(entry),
        disk_original: None,
        journal,
        message: None,
    }
}

pub fn response_conflict(disk: String) -> SaveResponse {
    SaveResponse {
        saved: false,
        conflict: true,
        entry: None,
        disk_original: Some(disk),
        journal: None,
        message: Some(
            "This file changed on disk. Yoman did not overwrite either version.".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("yoman-write-{nanos}-{id}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-16T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn two_entries_on_the_same_date_do_not_overwrite() {
        let dir = temp_dir();
        let first = create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        let second =
            create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        assert_eq!(first.file_name, "2026-10-16-100000-untitled.md");
        assert!(second.file_name.starts_with("2026-10-16-100000-untitled-"));
        assert_ne!(first.file_name, second.file_name);
        assert!(first.original.contains("Untitled"));
        assert!(second.original.contains("Untitled"));
        assert_eq!(
            fs::read_to_string(dir.join(&first.file_name)).unwrap(),
            first.original
        );
        assert_eq!(
            fs::read_to_string(dir.join(&second.file_name)).unwrap(),
            second.original
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn title_edits_rename_the_file_and_keep_unknown_frontmatter() {
        let dir = temp_dir();
        let created =
            create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        let original = format!(
            "---\nid: \"stable\"\ntitle: \"Untitled\"\ncreated: \"2026-10-16T10:00:00Z\"\nmood: \"שקט\"\n---\nhello\n"
        );
        fs::write(dir.join(&created.file_name), &original).unwrap();
        let saved = save_entry(
            &dir,
            &created.file_name,
            "My day",
            "היום עבדתי\n",
            &original,
            "2026-10-16T10:00:00Z",
            &[
                "  אישי  ".to_string(),
                "אישי".to_string(),
                "work".to_string(),
            ],
        )
        .unwrap();
        let SaveOutcome::Saved { entry } = saved else {
            panic!("expected save");
        };
        assert_eq!(entry.file_name, "2026-10-16-100000-my-day.md");
        assert!(!dir.join(&created.file_name).exists());
        assert_eq!(entry.title, "My day");
        assert!(entry.body.contains("היום עבדתי"));
        assert!(
            entry
                .extra
                .iter()
                .any(|field| field.key == "mood" && field.value == "שקט")
        );
        assert_eq!(entry.tags, vec!["אישי".to_string(), "work".to_string()]);
        assert!(entry.original.contains("id: stable") || entry.original.contains("id: \"stable\""));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn external_change_is_not_overwritten() {
        let dir = temp_dir();
        let created =
            create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        let changed = created.original.replace("Untitled", "From disk");
        fs::write(dir.join(&created.file_name), &changed).unwrap();
        let result = save_entry(
            &dir,
            &created.file_name,
            "Mine",
            "editor text\n",
            &created.original,
            "2026-10-16T10:00:00Z",
            &[],
        )
        .unwrap();
        match result {
            SaveOutcome::Conflict { disk } => assert_eq!(disk, changed),
            SaveOutcome::Saved { .. } => panic!("overwrite"),
        }
        assert_eq!(
            fs::read_to_string(dir.join(&created.file_name)).unwrap(),
            changed
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn copy_keeps_the_original_file() {
        let dir = temp_dir();
        let created =
            create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        fs::write(dir.join(&created.file_name), "external\n").unwrap();
        let copy = save_copy(
            &dir,
            "Mine",
            "my version\n",
            &created.original,
            now(),
            "2026-10-16-100000",
            "2026-10-16T10:00:00Z",
            &[],
        )
        .unwrap();
        assert_ne!(copy.file_name, created.file_name);
        assert_eq!(
            fs::read_to_string(dir.join(&created.file_name)).unwrap(),
            "external\n"
        );
        assert!(copy.original.contains("my version"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_paths_outside_the_folder() {
        let dir = temp_dir();
        let err = save_entry(
            &dir,
            "../secret.md",
            "T",
            "body\n",
            "x",
            "2026-10-16T10:00:00Z",
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, WriteError::InvalidName));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_deleted_file_is_not_recreated() {
        let dir = temp_dir();
        let created =
            create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        fs::remove_file(dir.join(&created.file_name)).unwrap();
        let err = save_entry(
            &dir,
            &created.file_name,
            "Still here",
            "unsaved text\n",
            &created.original,
            "2026-10-16T10:00:00Z",
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, WriteError::Unreadable { .. }));
        assert!(!dir.join(&created.file_name).exists());
        assert!(fs::read_dir(&dir).unwrap().next().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unwritable_folder_does_not_report_success() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        let mut perms = fs::metadata(&dir).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&dir, perms.clone()).unwrap();
        let err = create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z");
        perms.set_mode(0o755);
        fs::set_permissions(&dir, perms).unwrap();
        assert!(err.is_err());
        assert!(fs::read_dir(&dir).unwrap().next().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn delete_moves_only_the_named_file() {
        let dir = temp_dir();
        let created =
            create_entry(&dir, now(), "2026-10-16-100000", "2026-10-16T10:00:00Z").unwrap();
        let other = dir.join("keep.md");
        fs::write(&other, "keep\n").unwrap();
        delete_entry(&dir, &created.file_name).unwrap();
        assert!(!dir.join(&created.file_name).exists());
        assert_eq!(fs::read_to_string(&other).unwrap(), "keep\n");
        let err = delete_entry(&dir, "../keep.md").unwrap_err();
        assert!(matches!(err, WriteError::InvalidName));
        assert_eq!(fs::read_to_string(&other).unwrap(), "keep\n");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn slug_is_filesystem_safe() {
        assert_eq!(slugify("My day"), "my-day");
        assert_eq!(slugify("../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("יום טוב"), "יום-טוב");
        assert_eq!(slugify("   "), "untitled");
    }
}
