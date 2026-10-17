# Architecture decisions

Milestones 1 through 6. The functional spec is [functional-design.md](functional-design.md). File format, backups, and known limitations are in the [README](../README.md).

## Stack

The app uses Tauri 2, SvelteKit, Svelte 5, and TypeScript. `create-tauri-app` does not ship a SvelteKit preset, so the project was created with the Svelte CLI and then `tauri init`. SvelteKit 3 configures the static adapter in `vite.config.ts`. The UI is a single-page app (`ssr` and `prerender` are off, with an `index.html` fallback) because Tauri has no Node server.

Theme choice is a later milestone. Search and tags live in the window and are rebuilt from the Markdown files.

## Where journal files live

Markdown files live directly in the folder the user selects. There is no `entries/` subdirectory, and listing does not enter subfolders. The last folder is stored in the OS application config directory as `settings.json` for the identifier `app.yoman.desktop`. That file is not a second copy of the journal.

Settings are written by creating a temporary file beside `settings.json` and renaming it into place. A settings file that cannot be parsed is left as it is and reported in the window.

## Filesystem boundary

The frontend cannot choose an arbitrary path to read.

- `journal_status` and `choose_journal_folder` take no path arguments. `read_entry` takes only a file name, which Rust joins to the saved folder. Names with path separators, and files whose real path leaves the folder, are rejected.
- The folder comes from the native folder picker or from the saved settings file.
- Listing canonicalizes the folder and skips any item whose canonical path is outside it.
- The default capability grants `core:default` and `core:window:allow-destroy`. Destroy is used only after a close-time save succeeds. The capability does not grant `fs:`, `dialog:`, `shell:`, or `http:` commands.
- `@tauri-apps/plugin-fs` and `@tauri-apps/plugin-dialog` are not JavaScript dependencies. The dialog crate is used from Rust.
- `tauri-plugin-fs` is pulled in transitively by `tauri-plugin-dialog`. The app does not register that plugin, so its commands are not exposed.

The entry list is sorted by the frontmatter `date` or `created` value, newest first. A file without a usable date is sorted by its modification time. Names that are not valid UTF-8, files that are not valid UTF-8, and symlinks that leave the folder, are shown as notices instead of being dropped silently. Other file types are ignored.

Opening an entry parses YAML frontmatter for the title, date, id, and tags. Unknown fields are shown and are not removed from the file. If the frontmatter cannot be parsed, the window shows the original file text and a warning. A file that is not valid UTF-8 produces an error and is not replaced with an empty document.

The body is parsed with `pulldown-cmark` for the read-only path. The editor uses `mdast-util-from-markdown` and `mdast-util-to-markdown` (both MIT) so Markdown can be loaded into Tiptap and serialized again. Tiptap packages in use are MIT: `@tiptap/core`, `@tiptap/starter-kit`, `@tiptap/extension-link`, and `@tiptap/pm`. Strike and underline are disabled because this milestone does not serialize them.

Paragraphs default to `dir="auto"` with `unicode-bidi: plaintext`, so Hebrew and English follow the browser’s bidirectional algorithm. List markers do not follow that plaintext alignment, so each list item’s marker is placed from the item’s own direction: Hebrew items get `dir="rtl"` in the editor, and an empty item follows the Hebrew block above it. That direction is not written into the file unless you set the block explicitly. An explicit right-to-left or left-to-right choice is stored as a `<div dir="rtl">` or `<div dir="ltr">` wrapper around that block. Tests check that the wrapper round-trips and that the characters stay in logical order. Files that contain raw HTML, images, or task lists stay read-only, with the original text on screen, so the editor cannot drop them.

Saving writes the Markdown file through Rust. The frontend still cannot pick an arbitrary path. `create_entry` takes no path. `save_entry` takes a file name, the title, the body, the exact text that was loaded (`original`), and a mode: `update`, `overwrite`, or `copy`.

A new entry is `YYYY-MM-DD-HHMMSS-slug.md` in the selected folder. The date and time are the computer’s local time when the file is created. The slug keeps letters and numbers, including Hebrew, lowercases them, and turns other characters into dashes. If that name already exists, Yoman adds a short suffix and creates a different file. It does not overwrite. On Linux the final rename uses `renameat2` with `RENAME_NOREPLACE`.

The file is written to a temporary sibling whose name does not end in `.md`, flushed, then renamed into place. Before an ordinary save, Rust reads the current file and compares it to the loaded text. If they differ, nothing is written and the window offers three actions: reload the disk version, keep the editor version (replace the file), or save the editor version as a new file. Unknown frontmatter fields and tags stay in the file. `title` and `updated` change. `created` is added only when the file has neither `created` nor `date`. When the title changes, Yoman renames the file so the slug matches the new title, and keeps the original date and time in the name. A title of “My day” replaces `untitled` with `my-day`. The previous file is removed only after the new file is written. `updated` is an ISO 8601 timestamp with the local offset.

The editor waits about 700ms after the last change, then saves. The window says “Waiting to save…”, “Saving…”, “Saved”, or the error text. A conflict is also written out, not signaled by color alone. Autosave does not run while a conflict is open, and it does not rewrite a file that is shown unchanged (malformed frontmatter, raw HTML, images, or task lists). A failed save leaves the editor text in place.

There is no filesystem watcher. The app reads the file again when the window gains focus and again immediately before each save. If the editor matches the loaded text and the disk copy changed, the window reloads it. If the editor has unsaved text and the disk copy changed, the conflict actions appear.

Closing the window flushes a pending save. The window stays open when the save fails or a conflict is unresolved. Canonical formatting uses `-` lists, `*italic*`, `**bold**`, and `---` rules, so the first save may rewrite cosmetic Markdown. Arrow-key behavior in mixed Hebrew and English was not exercised in an automated browser; the automated checks cover stored text, paste insertion, marks, and direction metadata.

## Interface

The window is a sidebar plus the editor. The sidebar shows the folder, search, tag filters, new entry, and the entry list by title. Search is a case-insensitive substring over the title, body, and tags, including Hebrew, and it never writes a file. Tags are edited on the open entry and saved with the same atomic write and conflict check as the title and body. Duplicate tags that differ only by surrounding spaces are stored once, with the spelling kept. Delete asks for confirmation and moves that one file to the operating-system trash with the `trash` crate. If that fails, the entry stays in the list. The interface language is English. File names use `dir="auto"` so Hebrew names keep their own direction. The window uses a dark blue and deep purple palette. There is no separate theme control.

The content security policy allows the local Vite dev server and Tauri's IPC. It does not allow remote script or font hosts.

## Direct dependency licenses

Verified from each package's own metadata on 16 October 2026.

| Package                        | Version | License           |
| ------------------------------ | ------- | ----------------- |
| `@tauri-apps/api`              | 2.12.4  | Apache-2.0 OR MIT |
| `@tauri-apps/cli`              | 2.12.3  | Apache-2.0 OR MIT |
| `@sveltejs/kit`                | 3.1.0   | MIT               |
| `@sveltejs/adapter-static`     | 4.0.0   | MIT               |
| `@sveltejs/vite-plugin-svelte` | 7.3.1   | MIT               |
| `svelte`                       | 5.57.2  | MIT               |
| `svelte-check`                 | 4.7.6   | MIT               |
| `typescript`                   | 6.0.3   | Apache-2.0        |
| `vite`                         | 8.3.4   | MIT               |
| `vitest`                       | 4.1.12  | MIT               |
| `eslint`                       | 10.12.0 | MIT               |
| `@eslint/js`                   | 10.0.1  | MIT               |
| `eslint-config-prettier`       | 10.1.8  | MIT               |
| `eslint-plugin-svelte`         | 3.23.1  | MIT               |
| `typescript-eslint`            | 8.71.2  | MIT               |
| `globals`                      | 17.13.0 | MIT               |
| `prettier`                     | 3.9.10  | MIT               |
| `prettier-plugin-svelte`       | 4.1.1   | MIT               |
| `@types/node`                  | 24.19.2 | MIT               |
| `tauri`                        | 2.12.4  | Apache-2.0 OR MIT |
| `tauri-build`                  | 2.7.3   | Apache-2.0 OR MIT |
| `tauri-plugin-dialog`          | 2.8.3   | Apache-2.0 OR MIT |
| `tauri-plugin-log`             | 2.10.1  | Apache-2.0 OR MIT |
| `serde`                        | 1.0.229 | MIT OR Apache-2.0 |
| `serde_json`                   | 1.0.154 | MIT OR Apache-2.0 |
| `log`                          | 0.4.34  | MIT OR Apache-2.0 |
| `@tiptap/core`                 | 3.32.0  | MIT               |
| `@tiptap/starter-kit`          | 3.32.0  | MIT               |
| `@tiptap/extension-link`       | 3.32.0  | MIT               |
| `@tiptap/pm`                   | 3.32.0  | MIT               |
| `mdast-util-from-markdown`     | 2.1.0   | MIT               |
| `mdast-util-to-markdown`       | 2.2.0   | MIT               |
| `happy-dom`                    | 21.0.3  | MIT               |
| `pulldown-cmark`               | 0.13.4  | MIT               |
| `serde_yaml`                   | 0.9.34  | MIT OR Apache-2.0 |
| `chrono`                       | 0.4.45  | MIT OR Apache-2.0 |
| `libc`                         | 0.2.190 | MIT OR Apache-2.0 |
| `trash`                        | 5.2.9   | MIT               |

No paid Tiptap packages or hosted services are included.
