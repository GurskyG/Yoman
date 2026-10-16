# Yoman — Functional Design Document

**Document status:** MVP specification  
**Product type:** Local-first desktop journaling application  
**Primary platform:** Linux desktop  
**Working name:** Yoman (יומן)  
**Audience:** Cursor AI coding assistant and project contributors

---

## 1\. Product vision

Yoman is a private, local-first journaling application for writing and organizing personal journal entries. Journal entries are stored as ordinary Markdown (`.md`) files in a folder chosen by the user on their local drive.

The application must provide a comfortable writing experience for Hebrew and English, including paragraphs containing both languages. Correct right-to-left (RTL) and left-to-right (LTR) behavior is a core requirement, not a later enhancement.

The app must not require a cloud account, remote database, subscription, or proprietary API to perform its core functions.

### 1.1 Product principles

1. **The user's files are the source of truth.** Journal entries are Markdown files on disk, not records in a hidden database.  
2. **Local-first and private by default.** Core features work offline. No journal content is sent to external services.  
3. **Portable data.** Users can read and edit their entries with other Markdown editors.  
4. **Bilingual by design.** Hebrew, English, and mixed-direction content must be supported.  
5. **Small, reliable MVP.** Implement core writing and file safety before advanced features.  
6. **Explicit dependencies.** Use open-source components and verify licenses before adding dependencies.  
7. **Incremental development.** Make small changes, test them, and preserve working functionality.

---

## 2\. MVP scope

### 2.1 Included

- Desktop application shell.  
- Cross-platform: Windows, macOS, and Linux.  
- Simple installation: no Node.js, Rust, or developer tools required for end users.  
- Zero-account setup: no registration or sign-in.  
- First-run wizard: choose or create a journal folder.  
- Free core functionality: no paid extensions or required external services.  
- Portable journal: users retain direct ownership of their Markdown files.  
- User selects a journal folder on the local filesystem.  
- Open and browse Markdown entries in that folder.  
- Create, edit, rename where safe, and delete entries.  
- Rich-text editing with Markdown-compatible content.  
- Automatic saving with visible save status.  
- Entry list sorted by date, newest first by default.  
- Search entry titles and content.  
- Basic tags stored in Markdown frontmatter.  
- Hebrew RTL, English LTR, and mixed Hebrew/English editing.  
- Explicit paragraph direction controls where automatic direction is insufficient.  
- Detect external file changes and avoid silently overwriting external edits.  
- Export/backup by copying the journal folder; provide clear instructions.  
- Automated tests for file operations, editor behavior, and RTL/LTR cases.

### 2.2 Out of scope for MVP

Do not implement these unless explicitly requested later:

- Cloud synchronization or online accounts.  
- A server or database backend.  
- AI writing, summaries, embeddings, or semantic search.  
- Mobile applications.  
- Multi-user collaboration.  
- Calendar integrations, reminders, or notifications.  
- Images and attachments embedded into entries (design may allow future support).  
- Encryption or password-locking features.  
- Plugins, marketplace, or paid features.  
- Proprietary document formats.  
- Automatic Git repository creation or commits.

---

## 3\. Required technology stack

Use the following stack unless a genuine compatibility issue requires a documented alternative.

- **Desktop runtime:** Tauri 2\.  
- **Frontend:** SvelteKit, Svelte, TypeScript with strict type checking.  
- **Rich-text editor:** Tiptap / ProseMirror, using open-source packages only.  
- **Native filesystem access:** Rust commands exposed through Tauri.  
- **Markdown serialization:** Tiptap/ProseMirror Markdown-compatible parser and serializer, selected after verifying maintenance, compatibility, and license. If a dedicated package is unsuitable, implement a small, tested adapter around an established open-source Markdown parser/serializer.  
- **Testing:** Vitest for unit tests; Playwright for browser/UI tests where compatible with the Tauri setup; add Rust tests for native file operations.  
- **Styling:** CSS with logical properties and minimal dependencies.  
- **Package manager:** npm, unless the generated project already uses another package manager consistently.

### 3.1 Dependency and license rules

- Prefer permissive open-source licenses where practical (MIT, Apache-2.0, BSD).  
- Verify the license of each direct dependency before adding it.  
- Do not use paid Tiptap extensions or proprietary services.  
- Do not add a dependency for a feature that can be implemented simply and safely without one.  
- Record important dependency and architecture decisions in project documentation.

### 3.2 Architecture note

Tauri is required because this application must reliably read and write files in a user-selected local folder. A browser-only app is not the target architecture.

Keep filesystem access in a narrow Rust command layer. Do not grant unrestricted filesystem access to arbitrary frontend code. Validate all paths and operations on the native side.

---

## 4\. User personas and primary workflow

### 4.1 Primary user

A single person who wants to keep a private journal on their own computer and may write in Hebrew, English, or both in one entry.

### 4.2 Primary workflow

1. Launch Yoman.  
2. On first launch, select an existing journal folder or create/select a new folder.  
3. View entries in the sidebar.  
4. Open an entry or create a new one.  
5. Write in the editor, switching between Hebrew and English naturally.  
6. See a clear saving indicator.  
7. Close the application and reopen it later; the entry is still present as a Markdown file.  
8. Optionally open the Markdown file in another editor or back up the journal folder.

---

## 5\. Information architecture and UI

### 5.1 Main window

Use a calm, minimal desktop layout:

- **Sidebar:** app name, current journal folder name, search field, new-entry button, entry list, and a way to change the journal folder.  
- **Main editor area:** entry title, date/metadata, formatting toolbar, and editor body.  
- **Status area:** saving/saved/error state and current file location where useful.

The interface must be responsive to window resizing and usable at typical laptop sizes. Avoid excessive animation or decorative UI.

### 5.2 Empty states

- No journal folder configured: explain that the user must select a folder.  
- Folder contains no entries: explain how to create the first entry.  
- Search returns no results: show a clear no-results message and allow clearing the query.  
- Entry cannot be read: show an actionable error and do not replace its contents with an empty document.  
- Save fails: retain the user's current text in the editor and show a visible retry/error state.

### 5.3 Accessibility

- Use semantic controls and accessible labels.  
- All main actions must be usable by keyboard.  
- Provide visible focus indicators.  
- Respect operating-system reduced-motion preferences.  
- Do not rely on color alone to communicate save state or errors.  
- Tooltips may supplement labels but must not replace accessible names.

---

## 6\. Journal folder and file management

### 6.1 Choosing the journal folder

On first launch, prompt the user to select a folder using the native folder picker.

The app must:

- Store the chosen folder path in application settings.  
- Clearly show which folder is active.  
- Allow the user to choose a different folder later.  
- Never silently move or delete files when changing folders.  
- Reopen the last selected folder on subsequent launches if it still exists and is accessible.  
- Explain how to reselect the folder if it has been moved, disconnected, or become inaccessible.

Settings may be stored in a small application configuration file or Tauri-supported app configuration storage. Settings are not a substitute for entry storage; the Markdown files remain the source of truth.

### 6.2 Entry file format and naming

- Every entry is a standalone UTF-8 `.md` file.  
- Use a stable, filesystem-safe filename.  
- Recommended filename pattern: `YYYY-MM-DD-slug.md`; when multiple entries share a date, add a unique suffix.  
- Never use the title alone as a unique identifier.  
- Preserve existing filenames when editing a title unless the user explicitly renames the file or a clearly documented rename option is used.  
- Prevent path traversal and reject paths that escape the selected journal folder.  
- Do not overwrite an existing file when creating an entry.

Example:

```
Yoman/
└── entries/
    ├── 2026-10-16-my-day.md
    └── 2026-10-16-project-notes-a83f.md
```

&nbsp;

For MVP simplicity, the chosen journal folder may itself contain entry files; if an `entries/` subfolder is used, it must be created deliberately and explained to the user. Choose one approach during implementation and document it. Do not mix both layouts unpredictably.

### 6.3 Markdown frontmatter

Use YAML frontmatter for metadata, followed by standard Markdown content.

Example:

```markdown
---
id: "a-stable-unique-id"
title: "My day"
date: "2026-10-16T13:00:00"
tags:
  - personal
  - ideas
---

# My day

היום עבדתי על פרויקט חדש.

Today I made progress on my journaling app.
```

&nbsp;

Requirements:

- `title`: string.  
- `date`: ISO 8601 date/time string; choose and document whether it represents creation time or last-updated time. Prefer separate `created` and `updated` fields if both are needed.  
- `id`: stable unique identifier generated by the app. If an existing Markdown file has no `id`, the app may derive a stable identifier from its canonical path for the current session, but must not silently rewrite the file solely to add metadata unless the user edits/saves it or agrees to migration.  
- `tags`: optional list of strings.  
- Unknown frontmatter fields must be preserved whenever feasible; never discard user metadata without warning.

Do not store the same content in a separate database. A disposable in-memory index is allowed for performance, but it must be rebuildable from files and must not become a second source of truth.

### 6.4 Discovering entries

- Scan only the selected journal directory (and its documented entry subdirectory, if applicable).  
- Include `.md` files, case-insensitively.  
- Ignore unrelated file types.  
- Do not recurse into nested directories unless that behavior is explicitly designed and tested.  
- Handle invalid UTF-8 or malformed frontmatter gracefully; report the affected file and avoid data loss.  
- Sort by the entry date when valid; otherwise use filesystem modification time as a fallback and indicate if necessary.  
- Do not silently skip files without providing an understandable way to diagnose the problem.

### 6.5 Create an entry

When the user selects **New entry**:

1. Generate a unique stable ID and a safe filename.  
2. Set title to a useful default such as `Untitled`.  
3. Set creation date/time.  
4. Create the file without overwriting an existing file.  
5. Open it in the editor and focus the title or writing area.  
6. Mark it saved only after the file write succeeds.

If file creation fails, do not display a false success state. Show an actionable error.

### 6.6 Edit and save an entry

- Update the editor as the user types.  
- Debounce automatic saving; target approximately 500–1000 ms after the last edit.  
- Display `Saving…`, `Saved`, or an actionable error state.  
- Save atomically where possible: write a temporary file in the same directory, flush as appropriate, then replace the target file safely.  
- Avoid unnecessary writes when the serialized content has not changed.  
- Never discard unsaved editor content after a failed save.  
- On app close, attempt to flush pending writes. If changes cannot be saved, warn the user and offer to retry or keep the app open.  
- Do not claim a save succeeded until the native write operation reports success.

### 6.7 External changes and conflicts

Users may edit Markdown files using another application.

- Watch for external file changes where practical.  
- If the currently open file changes on disk while the editor has unsaved modifications, do not silently overwrite either version.  
- Show a conflict message with options such as **Reload from disk**, **Keep my current version**, and **Save my version as a copy**.  
- Before writing, compare the file's last-known content hash or equivalent revision marker with the current on-disk version.  
- If a conflict is detected, preserve both versions until the user chooses an action.  
- If filesystem watching is unreliable on a platform, re-check file metadata/content before save and when the window regains focus.

### 6.8 Delete an entry

- Require confirmation before deleting a file.  
- Show the filename or title in the confirmation.  
- Prefer moving the entry to the operating system's trash if a reliable cross-platform open-source implementation is available. Otherwise, clearly warn before permanent deletion.  
- Do not recursively delete directories.  
- If deletion fails, keep the entry visible and report the error.

### 6.9 Renaming

For MVP, editing the displayed title should not automatically rename the file. This avoids broken references and collisions. If file renaming is included, make it an explicit action, handle collisions, and update the active entry path only after a successful filesystem operation.

---

## 7\. Editor behavior and formatting

### 7.1 Core editing features

Provide these features using free/open-source editor extensions only:

- Paragraphs and headings.  
- Bold and italic.  
- Bulleted and numbered lists.  
- Blockquotes.  
- Links.  
- Undo and redo.  
- Keyboard navigation and common shortcuts.  
- Basic clipboard copy/paste.  
- Title editing separate from body content.  
- Optional horizontal rule if it serializes cleanly to Markdown.

Do not introduce advanced layout features that Markdown cannot represent reliably in MVP.

### 7.2 Markdown round-trip requirements

The app must parse Markdown into the editor and serialize editor content back to Markdown.

- Standard formatting must survive save, close, and reopen.  
- Text content must not be lost, duplicated, reversed, or reordered.  
- Avoid unsupported editor nodes that silently disappear when serialized.  
- If unsupported Markdown syntax is encountered, preserve it where feasible or warn the user before a save could destroy it.  
- Keep serialization/parsing behavior covered by tests.  
- Avoid formatting churn: opening and saving an unchanged file should not cause gratuitous rewrites.

---

## 8\. Hebrew, English, RTL, and LTR requirements

This is a critical acceptance area.

### 8.1 Direction rules

- The UI language/layout may be RTL or LTR independently of the direction of a journal paragraph.  
- Do not assume that the entire entry has one direction.  
- Default paragraph direction should be automatic where the editor supports it, using browser Unicode bidirectional behavior.  
- Provide explicit RTL and LTR controls for the current paragraph/block when automatic detection is insufficient.  
- Use HTML `dir` semantics and editor direction attributes where appropriate; do not reverse text strings.  
- Use CSS logical properties (`margin-inline-start`, `padding-inline-end`, `text-align: start`, etc.) rather than hard-coded left/right assumptions.

### 8.2 Mixed-direction content

Support a single paragraph such as:

`היום עבדתי על פרויקט חדש ב-Cursor וכתבתי קוד ב-TypeScript.`

Also support a paragraph containing English and Hebrew in either order, with punctuation, parentheses, numbers, dates, file paths, URLs, and technical terms.

The app must not manually reorder Unicode characters or strip bidirectional control characters indiscriminately. Use the browser's Unicode bidirectional algorithm and appropriate HTML/editor direction metadata.

### 8.3 Lists and block elements

Test RTL and LTR for:

- Headings.  
- Bulleted and numbered lists.  
- Nested lists.  
- Blockquotes.  
- Links and URLs.  
- Bold and italic text.  
- Paragraphs that start with numbers or punctuation.  
- Empty paragraphs and new paragraphs after pressing Enter.  
- Pasting text from other applications.

List markers, indentation, cursor placement, and selection must remain usable in both directions.

### 8.4 Cursor, selection, and keyboard behavior

Manually verify:

- Arrow keys move the cursor naturally around Hebrew and English runs.  
- Shift+arrow selection works in mixed-direction text.  
- Home/End behavior is usable.  
- Backspace and Delete do not corrupt mixed-direction text.  
- Enter creates a correctly directed next paragraph according to the editor's documented behavior.  
- Copy/paste preserves text and supported formatting.  
- Undo/redo restores expected text and formatting.  
- Switching the current paragraph's direction does not reverse or mutate its content.

Do not claim full Microsoft Word parity until tested. The target is robust everyday mixed Hebrew/English writing, with limitations documented honestly.

### 8.5 Persisting direction in Markdown

Standard Markdown does not define paragraph direction metadata.

Choose a portable strategy and document its trade-offs:

1. Use automatic direction for normal paragraphs, which requires no special syntax.  
2. For explicit direction overrides, use a documented HTML `dir` wrapper only if the chosen Markdown parser/serializer preserves it safely and round-trips correctly.  
3. If HTML wrappers are not reliably preserved, use a clearly documented, reversible convention or restrict explicit direction overrides to the editing session until a portable format is implemented. Do not silently lose explicit direction on save.

Test the chosen strategy with real files and the selected Markdown parser. Never add invisible control characters or custom syntax without documenting the portability implications.

---

## 9\. Search and tags

### 9.1 Search

- Search entry titles and body text across all recognized entries in the selected journal.  
- Search should be case-insensitive for Latin text.  
- Hebrew search must work normally.  
- Do not require a network connection.  
- Update results after a short debounce.  
- Show matching entries in the sidebar.  
- Clicking a result opens the entry.  
- Search indexing is in-memory and rebuildable from Markdown files.  
- Search must not modify entry files.

MVP search may be simple substring search; no embeddings or semantic search.

### 9.2 Tags

- Tags are optional frontmatter metadata.  
- Allow adding/removing tags in the UI.  
- Preserve tag spelling and Unicode.  
- Avoid duplicate tags that differ only by accidental surrounding whitespace.  
- Selecting a tag filters the entry list.  
- Save tag changes through the same safe file-write/conflict mechanism as other changes.

---

## 10\. Application settings and privacy

Minimal settings:

- Last selected journal folder.  
- Theme preference (system/light/dark) if implemented in MVP.  
- Any editor preference that is necessary for reliable use.

Privacy requirements:

- No telemetry by default.  
- No network requests for journal contents.  
- No external fonts or scripts required at runtime unless bundled locally and appropriately licensed.  
- Do not log entry content in application logs, errors, or test output.  
- Use Tauri's capability and permission model with least privilege.  
- Expose only the filesystem commands needed for the selected journal folder.  
- Validate all incoming native command arguments.  
- Treat Markdown content as untrusted text; never render arbitrary HTML without safe sanitization and a deliberate security decision.  
- Do not run Markdown content as code.

---

## 11\. Error handling and recovery

The app must handle:

- Missing or moved journal folder.  
- Read/write permission errors.  
- Disk full or write failure.  
- Existing filename collisions.  
- Malformed Markdown or frontmatter.  
- External file modifications.  
- File deletion or rename outside the app.  
- Unexpected shutdown during save.  
- Invalid or inaccessible filesystem paths.

Rules:

- Errors must be visible and actionable.  
- Never silently replace a file with an empty document because parsing failed.  
- Preserve unsaved text in memory after save failure.  
- Use atomic writes where possible.  
- Avoid destructive automatic repair.  
- Provide recovery guidance, including locating the journal folder and making a manual backup.  
- Do not report success until the operation has actually succeeded.

---

## 12\. Testing strategy and acceptance criteria

### 12.1 Project foundation

- The app starts in development mode.  
- The desktop shell opens successfully.  
- Type checking, linting, and tests run using documented commands.  
- No unrelated or proprietary service is required.

### 12.2 File operations

- Selecting a journal folder works.  
- Existing Markdown files are discovered.  
- A new entry creates a valid `.md` file.  
- Editing and saving updates the correct file.  
- Closing and reopening preserves the entry.  
- Two entries on the same date do not overwrite each other.  
- Deleting an entry follows the confirmed deletion behavior.  
- Invalid paths cannot escape the chosen journal folder.  
- Write failures do not cause data loss or false success.  
- External changes trigger conflict handling rather than silent overwrite.

### 12.3 Markdown integrity

- Headings, emphasis, lists, quotes, and links survive a save/reopen cycle.  
- Unicode and Hebrew text are preserved byte-for-byte where content is unchanged, subject to documented line-ending/serialization behavior.  
- Unknown frontmatter fields are not silently discarded.  
- Unsupported syntax is handled safely and documented.  
- Unchanged files do not undergo unnecessary serialization changes.

### 12.4 RTL/LTR test cases

Include automated tests for serialization and browser-level tests where feasible. Manually verify actual visual and cursor behavior.

Test at least these strings:

1. `שלום עולם`  
2. `Hello world`  
3. `שלום from Cursor`  
4. `Today כתבתי TypeScript בקוד`  
5. `המחיר הוא 123.45 ש"ח`  
6. `פתחתי את הקובץ (README.md) בפרויקט`  
7. `https://example.com שלום`  
8. A Hebrew bulleted list containing English product names.  
9. An English numbered list containing Hebrew phrases.  
10. A paragraph with parentheses, quotation marks, punctuation, and digits in both scripts.

For each relevant case, verify:

- Visual reading order is sensible.  
- Characters are stored in logical Unicode order.  
- Cursor movement and selection work.  
- Copy/paste preserves text.  
- Save/reopen preserves content and supported direction metadata.  
- Text is not duplicated, reversed, or corrupted.

### 12.5 Search and tags

- Search finds Hebrew and English text.  
- Search is case-insensitive for Latin text.  
- Tags persist after restart.  
- Tag filters return the correct entries.

---

## 13\. Suggested implementation milestones

Implement one milestone at a time. Do not attempt the entire app in one generation.

### Milestone 1 — Scaffold and native file access

- Create the Tauri 2 \+ SvelteKit \+ TypeScript project.  
- Run the app locally.  
- Implement native folder selection and a safe read-only listing of `.md` files.  
- Confirm the frontend cannot access arbitrary filesystem paths directly.  
- Add basic tests.

**Exit condition:** the user can select a journal folder and see its Markdown files.

### Milestone 2 — Read and display Markdown

- Read one selected entry.  
- Parse frontmatter and Markdown safely.  
- Display title and content.  
- Handle malformed files without losing their original content.

**Exit condition:** existing journal files can be opened read-only.

### Milestone 3 — Bilingual editor proof of concept

- Integrate Tiptap.  
- Implement automatic and explicit block direction as supported.  
- Test mixed Hebrew/English, lists, punctuation, cursor movement, and paste.  
- Implement Markdown round-trip tests before enabling write operations.

**Exit condition:** the core Hebrew/English editing tests pass sufficiently to proceed.

### Milestone 4 — Safe write and create operations

- Implement create entry, debounced save, atomic write, save status, and conflict detection.  
- Add explicit error states.  
- Verify no existing file can be overwritten by a generated filename collision.

**Exit condition:** entries can be created and edited without silent data loss.

### Milestone 5 — Complete core UI

- Add sidebar, entry list, title editing, toolbar, delete confirmation, search, and tags.  
- Implement empty states and responsive styling.

**Exit condition:** the app is usable for everyday journaling.

### Milestone 6 — Hardening and documentation

- Add tests for external changes, permissions, failures, and RTL/LTR edge cases.  
- Document file format, folder layout, backups, known limitations, and development commands.  
- Verify licenses and Tauri permissions.  
- Test the production build on Linux.

**Exit condition:** a stable, documented local-first MVP.

---

## 14\. Definition of done for every milestone

A milestone is complete only when:

1. The implementation matches the requirements in that milestone.  
2. Existing features still work.  
3. Appropriate automated tests have been executed.  
4. Manual browser/desktop checks have been completed where required.  
5. Actual test results are reported accurately.  
6. Any limitation or deviation is documented.  
7. The user can review the code changes and commit the working state.

Do not claim a test passed unless it was run and passed. If a command cannot run in the current environment, say so explicitly.

---

## 15\. Instructions to Cursor

Treat this document as the functional specification, not as permission to implement everything at once.

At the start:

1. Inspect the existing repository and report its current structure and status.  
2. Identify whether it already contains a SvelteKit or Tauri project.  
3. Propose the smallest implementation plan for **Milestone 1 only**.  
4. List dependencies to be added and their licenses where known.  
5. Wait for user approval before making substantial changes if the project structure or stack differs from this specification.

During development:

- Work only on the current milestone.  
- Keep changes small and reviewable.  
- Prefer clear, maintainable code over abstractions for hypothetical future features.  
- Never introduce a backend, cloud service, database, or paid dependency without approval.  
- Never replace Markdown files with another source of truth.  
- Never implement RTL by reversing text.  
- Run the relevant tests and report actual results.  
- Update this document or a separate decision log when an architectural decision changes.

**First task:** inspect the repository and prepare a plan for Milestone 1\. Do not implement later milestones yet.