use std::fs;
use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::Serialize;
use serde_yaml::Value;

use crate::journal::{self, JournalError};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedEntry {
    pub file_name: String,
    pub readable: bool,
    pub title: String,
    pub body: String,
    pub original: String,
    pub warning: Option<String>,
    pub date: Option<String>,
    pub updated: Option<String>,
    pub id: Option<String>,
    pub tags: Vec<String>,
    pub extra: Vec<FrontmatterField>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Quote {
        blocks: Vec<Block>,
    },
    Code {
        text: String,
    },
    Rule,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Inline {
    Text { text: String },
    Strong { inlines: Vec<Inline> },
    Emphasis { inlines: Vec<Inline> },
    Code { text: String },
    Link { href: String, inlines: Vec<Inline> },
    Break { hard: bool },
}

#[derive(Debug)]
pub enum EntryError {
    InvalidName,
    Outside,
    Missing { file_name: String },
    Unreadable { file_name: String, detail: String },
    Folder(JournalError),
}

impl EntryError {
    pub fn user_message(&self) -> String {
        match self {
            Self::InvalidName | Self::Outside => {
                "Yoman only opens a Markdown file directly inside the selected journal folder. No files were changed.".to_string()
            }
            Self::Missing { file_name } => format!(
                "\"{file_name}\" is no longer in the journal folder. No files were changed."
            ),
            Self::Unreadable { file_name, detail } => format!(
                "Yoman could not read \"{file_name}\" ({detail}). No files were changed."
            ),
            Self::Folder(err) => format!("{} No files were changed.", err.user_message()),
        }
    }
}

pub fn read_entry(journal_dir: &Path, file_name: &str) -> Result<OpenedEntry, EntryError> {
    let path = resolve_entry_path(journal_dir, file_name)?;
    let bytes = fs::read(&path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            EntryError::Missing {
                file_name: file_name.to_string(),
            }
        } else {
            EntryError::Unreadable {
                file_name: file_name.to_string(),
                detail: err.to_string(),
            }
        }
    })?;
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(unreadable_entry(
            file_name,
            "This file is not valid UTF-8 text, so Yoman is not showing it as an empty entry. No files were changed.",
        ));
    };
    Ok(parse_entry(file_name, &text))
}

pub fn parse_entry(file_name: &str, original: &str) -> OpenedEntry {
    let scan = original.strip_prefix('\u{feff}').unwrap_or(original);
    match split_frontmatter(scan) {
        FrontmatterSplit::Absent => opened_from_body(file_name, original, scan, None, None),
        FrontmatterSplit::Unclosed => opened_from_body(
            file_name,
            original,
            original,
            None,
            Some(
                "The frontmatter at the top of this file is not closed, so Yoman is showing the original file unchanged."
                    .to_string(),
            ),
        ),
        FrontmatterSplit::Closed { yaml, body } => match parse_frontmatter(yaml) {
            Ok(meta) => opened_from_body(file_name, original, body, Some(meta), None),
            Err(message) => opened_from_body(file_name, original, original, None, Some(message)),
        },
    }
}

fn unreadable_entry(file_name: &str, warning: &str) -> OpenedEntry {
    OpenedEntry {
        file_name: file_name.to_string(),
        readable: false,
        title: file_name.to_string(),
        body: String::new(),
        original: String::new(),
        warning: Some(warning.to_string()),
        date: None,
        updated: None,
        id: None,
        tags: Vec::new(),
        extra: Vec::new(),
        blocks: Vec::new(),
    }
}

fn opened_from_body(
    file_name: &str,
    original: &str,
    body: &str,
    meta: Option<Frontmatter>,
    warning: Option<String>,
) -> OpenedEntry {
    let meta = meta.unwrap_or_default();
    let title = meta
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| title_from_file_name(file_name));
    OpenedEntry {
        file_name: file_name.to_string(),
        readable: true,
        title,
        blocks: if warning.is_some() {
            Vec::new()
        } else {
            markdown_blocks(body)
        },
        body: body.to_string(),
        original: original.to_string(),
        warning,
        date: meta.date,
        updated: meta.updated,
        id: meta.id,
        tags: meta.tags,
        extra: meta.extra,
    }
}

#[derive(Default)]
struct Frontmatter {
    title: Option<String>,
    date: Option<String>,
    updated: Option<String>,
    id: Option<String>,
    tags: Vec<String>,
    extra: Vec<FrontmatterField>,
}

enum FrontmatterSplit<'a> {
    Absent,
    Unclosed,
    Closed { yaml: &'a str, body: &'a str },
}

fn split_frontmatter(text: &str) -> FrontmatterSplit<'_> {
    let Some(rest) = text.strip_prefix("---") else {
        return FrontmatterSplit::Absent;
    };
    let Some(rest) = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
    else {
        return FrontmatterSplit::Unclosed;
    };

    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return FrontmatterSplit::Closed { yaml, body };
        }
        offset += line.len();
    }
    FrontmatterSplit::Unclosed
}

fn parse_frontmatter(yaml: &str) -> Result<Frontmatter, String> {
    if yaml.trim().is_empty() {
        return Ok(Frontmatter::default());
    }
    let value: Value = serde_yaml::from_str(yaml).map_err(|_| {
        "The frontmatter in this file could not be read, so Yoman is showing the original file unchanged.".to_string()
    })?;
    let Some(mapping) = value.as_mapping() else {
        return Err(
            "The frontmatter in this file is not a set of fields, so Yoman is showing the original file unchanged.".to_string(),
        );
    };

    let mut meta = Frontmatter::default();
    for (key, value) in mapping {
        let key = yaml_key(key);
        match key.as_str() {
            "title" => meta.title = yaml_text(value),
            "date" | "created" if meta.date.is_none() => meta.date = yaml_text(value),
            "updated" => meta.updated = yaml_text(value),
            "id" => meta.id = yaml_text(value),
            "tags" => meta.tags = yaml_tags(value),
            _ => {
                if let Some(rendered) = yaml_text(value) {
                    meta.extra.push(FrontmatterField {
                        key,
                        value: rendered,
                    });
                }
            }
        }
    }
    meta.extra.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(meta)
}

fn yaml_key(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => yaml_text(other).unwrap_or_else(|| "field".to_string()),
    }
}

fn yaml_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        other => serde_yaml::to_string(other)
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
    }
}

fn yaml_tags(value: &Value) -> Vec<String> {
    let mut tags = Vec::new();
    match value {
        Value::String(text) => push_tag(&mut tags, text),
        Value::Sequence(items) => {
            for item in items {
                if let Value::String(text) = item {
                    push_tag(&mut tags, text);
                }
            }
        }
        _ => {}
    }
    tags
}

fn push_tag(tags: &mut Vec<String>, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() || tags.iter().any(|tag| tag == trimmed) {
        return;
    }
    tags.push(trimmed.to_string());
}

fn title_from_file_name(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| file_name.to_string())
}

fn resolve_entry_path(journal_dir: &Path, file_name: &str) -> Result<PathBuf, EntryError> {
    if !is_single_markdown_name(file_name) {
        return Err(EntryError::InvalidName);
    }
    if !journal_dir.exists() {
        return Err(EntryError::Folder(JournalError::Missing {
            path: journal_dir.to_path_buf(),
        }));
    }
    let root = journal_dir.canonicalize().map_err(|err| {
        EntryError::Folder(JournalError::Unreadable {
            path: journal_dir.to_path_buf(),
            detail: err.to_string(),
        })
    })?;
    if !root.is_dir() {
        return Err(EntryError::Folder(JournalError::NotDirectory {
            path: root,
        }));
    }
    let candidate = root.join(file_name);
    if !candidate.exists() {
        return Err(EntryError::Missing {
            file_name: file_name.to_string(),
        });
    }
    if !journal::is_inside_journal(&root, &candidate) {
        return Err(EntryError::Outside);
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|err| EntryError::Unreadable {
            file_name: file_name.to_string(),
            detail: err.to_string(),
        })?;
    if !canonical.is_file() {
        return Err(EntryError::InvalidName);
    }
    Ok(canonical)
}

fn is_single_markdown_name(file_name: &str) -> bool {
    if file_name.is_empty() || file_name.contains('\0') {
        return false;
    }
    let path = Path::new(file_name);
    let mut components = path.components();
    let Some(Component::Normal(_)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    journal::is_markdown_name(file_name)
}

fn markdown_blocks(markdown: &str) -> Vec<Block> {
    let mut stack = vec![Container::Root(Vec::new())];
    let mut inlines: Vec<Vec<Inline>> = Vec::new();
    let mut code: Option<String> = None;
    let mut html: Option<String> = None;

    for event in Parser::new(markdown) {
        if code.is_some() {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    push_block(
                        &mut stack,
                        Block::Code {
                            text: code.take().unwrap_or_default(),
                        },
                    );
                }
                Event::Text(text) => code.as_mut().unwrap().push_str(&text),
                Event::SoftBreak | Event::HardBreak => code.as_mut().unwrap().push('\n'),
                _ => {}
            }
            continue;
        }
        if html.is_some() {
            match event {
                Event::End(TagEnd::HtmlBlock) => {
                    push_block(
                        &mut stack,
                        Block::Paragraph {
                            inlines: vec![Inline::Text {
                                text: html.take().unwrap_or_default(),
                            }],
                        },
                    );
                }
                Event::Html(text) | Event::Text(text) => html.as_mut().unwrap().push_str(&text),
                _ => {}
            }
            continue;
        }

        match event {
            Event::Start(Tag::Paragraph) => inlines.push(Vec::new()),
            Event::End(TagEnd::Paragraph) => {
                let content = inlines.pop().unwrap_or_default();
                push_block(&mut stack, Block::Paragraph { inlines: content });
            }
            Event::Start(Tag::Heading { .. }) => inlines.push(Vec::new()),
            Event::End(TagEnd::Heading(level)) => {
                let content = inlines.pop().unwrap_or_default();
                push_block(
                    &mut stack,
                    Block::Heading {
                        level: heading_level(level),
                        inlines: content,
                    },
                );
            }
            Event::Start(Tag::BlockQuote(_)) => stack.push(Container::Quote(Vec::new())),
            Event::End(TagEnd::BlockQuote(_)) => {
                if let Some(Container::Quote(blocks)) = stack.pop() {
                    push_block(&mut stack, Block::Quote { blocks });
                }
            }
            Event::Start(Tag::List(first)) => stack.push(Container::List {
                ordered: first.is_some(),
                items: Vec::new(),
                item: Vec::new(),
            }),
            Event::End(TagEnd::List(_)) => {
                if let Some(Container::List {
                    ordered,
                    mut items,
                    item,
                    ..
                }) = stack.pop()
                {
                    if !item.is_empty() {
                        items.push(item);
                    }
                    push_block(&mut stack, Block::List { ordered, items });
                }
            }
            Event::Start(Tag::Item) => {
                if let Some(Container::List { item, items, .. }) = stack.last_mut() {
                    if !item.is_empty() {
                        items.push(std::mem::take(item));
                    }
                }
            }
            Event::End(TagEnd::Item) => {
                if let Some(Container::List { item, items, .. }) = stack.last_mut() {
                    items.push(std::mem::take(item));
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code = Some(String::new()),
            Event::Start(Tag::HtmlBlock) => html = Some(String::new()),
            Event::Start(Tag::Emphasis) => inlines.push(Vec::new()),
            Event::End(TagEnd::Emphasis) => wrap_inlines(&mut inlines, InlineKind::Emphasis),
            Event::Start(Tag::Strong) => inlines.push(Vec::new()),
            Event::End(TagEnd::Strong) => wrap_inlines(&mut inlines, InlineKind::Strong),
            Event::Start(Tag::Link { dest_url, .. }) => {
                inlines.push(vec![Inline::Text {
                    text: dest_url.to_string(),
                }]);
                inlines.push(Vec::new());
            }
            Event::End(TagEnd::Link) => finish_link(&mut inlines),
            Event::Start(Tag::Image { dest_url, .. }) => {
                inlines.push(vec![Inline::Text {
                    text: format!("image: {dest_url}"),
                }]);
                inlines.push(Vec::new());
            }
            Event::End(TagEnd::Image) => finish_image(&mut inlines),
            Event::Code(text) => push_inlines(
                &mut inlines,
                Inline::Code {
                    text: text.to_string(),
                },
            ),
            Event::Text(text) => push_inlines(
                &mut inlines,
                Inline::Text {
                    text: text.to_string(),
                },
            ),
            Event::Html(text) | Event::InlineHtml(text) => push_inlines(
                &mut inlines,
                Inline::Text {
                    text: text.to_string(),
                },
            ),
            Event::SoftBreak => push_inlines(&mut inlines, Inline::Break { hard: false }),
            Event::HardBreak => push_inlines(&mut inlines, Inline::Break { hard: true }),
            Event::Rule => push_block(&mut stack, Block::Rule),
            Event::TaskListMarker(checked) => push_inlines(
                &mut inlines,
                Inline::Text {
                    text: if checked { "[x] " } else { "[ ] " }.to_string(),
                },
            ),
            Event::FootnoteReference(label) => push_inlines(
                &mut inlines,
                Inline::Text {
                    text: format!("[{label}]"),
                },
            ),
            Event::InlineMath(text) => push_inlines(
                &mut inlines,
                Inline::Code {
                    text: text.to_string(),
                },
            ),
            Event::DisplayMath(text) => push_block(
                &mut stack,
                Block::Code {
                    text: text.to_string(),
                },
            ),
            Event::Start(Tag::FootnoteDefinition(label)) => push_inlines(
                &mut inlines,
                Inline::Text {
                    text: format!("[{label}]: "),
                },
            ),
            Event::End(TagEnd::FootnoteDefinition) | Event::Start(_) | Event::End(_) => {}
        }
    }

    match stack.pop() {
        Some(Container::Root(blocks)) => blocks,
        _ => Vec::new(),
    }
}

enum Container {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
        item: Vec<Block>,
    },
}

enum InlineKind {
    Strong,
    Emphasis,
}

fn push_block(stack: &mut [Container], block: Block) {
    match stack.last_mut() {
        Some(Container::Root(blocks) | Container::Quote(blocks)) => blocks.push(block),
        Some(Container::List { item, .. }) => item.push(block),
        None => {}
    }
}

fn push_inlines(inlines: &mut [Vec<Inline>], inline: Inline) {
    if let Some(current) = inlines.last_mut() {
        current.push(inline);
    }
}

fn wrap_inlines(inlines: &mut Vec<Vec<Inline>>, kind: InlineKind) {
    let content = inlines.pop().unwrap_or_default();
    let wrapped = match kind {
        InlineKind::Strong => Inline::Strong { inlines: content },
        InlineKind::Emphasis => Inline::Emphasis { inlines: content },
    };
    push_inlines(inlines, wrapped);
}

fn finish_link(inlines: &mut Vec<Vec<Inline>>) {
    let label = inlines.pop().unwrap_or_default();
    let href = inlines.pop().and_then(|mut marker| match marker.pop() {
        Some(Inline::Text { text }) => Some(text),
        _ => None,
    });
    let Some(href) = href else {
        for inline in label {
            push_inlines(inlines, inline);
        }
        return;
    };
    if let Some(safe) = safe_http_href(&href) {
        push_inlines(
            inlines,
            Inline::Link {
                href: safe,
                inlines: label,
            },
        );
    } else {
        push_inlines(
            inlines,
            Inline::Text {
                text: inline_text(&label),
            },
        );
        push_inlines(
            inlines,
            Inline::Text {
                text: format!(" ({href})"),
            },
        );
    }
}

fn finish_image(inlines: &mut Vec<Vec<Inline>>) {
    let alt = inlines.pop().unwrap_or_default();
    let marker = inlines.pop();
    let prefix = match marker.and_then(|mut marker| marker.pop()) {
        Some(Inline::Text { text }) => text,
        _ => "image".to_string(),
    };
    let alt_text = inline_text(&alt);
    let text = if alt_text.is_empty() {
        prefix
    } else {
        format!("{prefix} ({alt_text})")
    };
    push_inlines(inlines, Inline::Text { text });
}

fn inline_text(inlines: &[Inline]) -> String {
    let mut collected = String::new();
    for inline in inlines {
        match inline {
            Inline::Text { text } | Inline::Code { text } => collected.push_str(text),
            Inline::Link { href, inlines } => {
                collected.push_str(&inline_text(inlines));
                collected.push_str(href);
            }
            Inline::Strong { inlines } | Inline::Emphasis { inlines } => {
                collected.push_str(&inline_text(inlines));
            }
            Inline::Break { hard: true } => collected.push('\n'),
            Inline::Break { hard: false } => collected.push(' '),
        }
    }
    collected
}

fn safe_http_href(href: &str) -> Option<String> {
    let trimmed = href.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
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
            let path = std::env::temp_dir().join(format!("yoman-entry-{nanos}-{id}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_frontmatter_title_tags_and_unknown_fields() {
        let temp = TempDir::new();
        let journal = temp.0.join("journal");
        fs::create_dir_all(&journal).unwrap();
        let body = "היום עבדתי על פרויקט חדש.\n";
        let original = format!(
            "---\ntitle: \"יום\"\ndate: \"2026-10-16T13:00:00\"\nid: \"abc\"\nmood: \"שקט\"\ntags:\n  - אישי\n  - אישי\n---\n{body}"
        );
        fs::write(journal.join("day.md"), &original).unwrap();

        let opened = read_entry(&journal, "day.md").unwrap();

        assert!(opened.readable);
        assert_eq!(opened.title, "יום");
        assert_eq!(opened.date.as_deref(), Some("2026-10-16T13:00:00"));
        assert_eq!(opened.id.as_deref(), Some("abc"));
        assert_eq!(opened.tags, vec!["אישי".to_string()]);
        assert_eq!(
            opened.extra,
            vec![FrontmatterField {
                key: "mood".to_string(),
                value: "שקט".to_string(),
            }]
        );
        assert_eq!(opened.body, body);
        assert_eq!(opened.original, original);
        assert!(opened.warning.is_none());
        assert_eq!(
            fs::read_to_string(journal.join("day.md")).unwrap(),
            original
        );
    }

    #[test]
    fn malformed_frontmatter_keeps_the_original_text() {
        let original = "---\ntitle: [\nhello from the file\n";
        let opened = parse_entry("note.md", original);
        assert!(opened.readable);
        assert_eq!(opened.original, original);
        assert_eq!(opened.body, original);
        assert!(opened.warning.is_some());
        assert!(opened.blocks.is_empty());
    }

    #[test]
    fn rejects_names_that_leave_the_journal_folder() {
        let temp = TempDir::new();
        let journal = temp.0.join("journal");
        let outside = temp.0.join("outside");
        fs::create_dir_all(&journal).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(journal.join("inside.md"), "inside").unwrap();
        fs::write(outside.join("secret.md"), "secret").unwrap();
        fs::create_dir_all(journal.join("nested")).unwrap();
        fs::write(journal.join("nested/hidden.md"), "hidden").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.md"), journal.join("secret.md")).unwrap();

        assert!(matches!(
            read_entry(&journal, "../secret.md"),
            Err(EntryError::InvalidName)
        ));
        assert!(matches!(
            read_entry(&journal, "nested/hidden.md"),
            Err(EntryError::InvalidName)
        ));
        assert!(matches!(
            read_entry(&journal, "secret.md"),
            Err(EntryError::Outside)
        ));
        assert_eq!(
            fs::read_to_string(outside.join("secret.md")).unwrap(),
            "secret"
        );
        assert_eq!(
            fs::read_to_string(journal.join("inside.md")).unwrap(),
            "inside"
        );
    }

    #[test]
    fn invalid_utf8_is_not_presented_as_an_empty_document() {
        let temp = TempDir::new();
        let journal = temp.0.join("journal");
        fs::create_dir_all(&journal).unwrap();
        let bytes = b"---\ntitle: ok\n---\n\xFF".to_vec();
        fs::write(journal.join("bad.md"), &bytes).unwrap();

        let opened = read_entry(&journal, "bad.md").unwrap();

        assert!(!opened.readable);
        assert!(opened.body.is_empty());
        assert!(opened.original.is_empty());
        assert!(opened.warning.unwrap().contains("UTF-8"));
        assert_eq!(fs::read(journal.join("bad.md")).unwrap(), bytes);
    }

    #[test]
    fn markdown_text_is_kept_and_html_is_not_a_rendered_node() {
        let opened = parse_entry(
            "note.md",
            "# Hello\n\nToday **כתבתי** `TypeScript`.\n\n<script>alert(1)</script>\n",
        );
        assert_eq!(opened.title, "note");
        let paragraph = opened
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Paragraph { inlines } => Some(inlines),
                _ => None,
            })
            .unwrap();
        assert!(
            paragraph
                .iter()
                .any(|inline| matches!(inline, Inline::Strong { .. }))
        );
        let html = opened.blocks.iter().any(|block| match block {
            Block::Paragraph { inlines } => inlines.iter().any(|inline| {
                matches!(
                    inline,
                    Inline::Text { text } if text.contains("<script>")
                )
            }),
            _ => false,
        });
        assert!(html);
    }
}
