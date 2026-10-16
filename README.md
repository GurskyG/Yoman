# Yoman

Yoman is a local-first desktop journal. Entries are ordinary Markdown files in a folder you choose. You can create and edit entries in Hebrew or English, search them, tag them, and move an entry to the trash.

The functional specification is in [docs/functional-design.md](docs/functional-design.md). Architecture choices are in [docs/decisions.md](docs/decisions.md).

## Requirements

- Node.js and npm
- Rust (stable), via [rustup](https://rustup.rs/)
- Linux desktop libraries for Tauri: WebKitGTK 4.1 and the usual GTK build tools

## Commands

```sh
npm run tauri dev      # desktop app
npm run tauri build    # Linux production build
npm run check          # TypeScript
npm run lint           # Prettier and ESLint
npm test               # Vitest
npm run test:rust      # filesystem, save, and permission tests
```

`npm run dev` starts only the web UI. Folder selection, saving, and trash work in the desktop window, not in a browser tab.

## Journal folder

Markdown files live directly in the folder you select. Yoman does not create an `entries/` subdirectory and does not scan subfolders. The last selected folder is stored in the application settings file (`settings.json` in the OS config directory for `app.yoman.desktop`), not inside the journal. Settings remember the folder path only. They are not a copy of your entries.

## File format

Each entry is one UTF-8 `.md` file. The name looks like `2026-10-16-150432-my-day.md`: local date, local time, then a slug of the title. Hebrew letters stay in the slug. If that name already exists, Yoman adds a short suffix instead of overwriting. Changing the title renames the file and keeps the original date and time.

The file starts with YAML frontmatter:

```markdown
---
id: '…'
title: 'My day'
created: '2026-10-16T15:04:32+03:00'
updated: '2026-10-16T15:10:00+03:00'
tags:
  - אישי
---

The entry body is Markdown.
```

`created` is when the entry was first saved. `updated` changes on later saves. Unknown frontmatter fields are kept. A file whose frontmatter cannot be read is shown unchanged and is not rewritten. A file that is not valid UTF-8 is reported and is not opened as an empty entry.

## Backups

Copy the journal folder. That folder is the whole journal. Yoman does not upload it and does not keep a second copy.

If a save is interrupted, the previous file stays in place until the new contents are written. A leftover file whose name ends in `.yoman-tmp` is an unfinished write. It is not an entry. You can delete those leftovers after Yoman is closed. If the app could not save, the text is still in the editor and the window stays open.

To move the journal, copy the folder, then choose the new location with **Change folder**. Choosing a different folder does not move or delete the old files.

## Known limitations

- There is no account, sync, encryption, or database.
- The app does not watch the folder. It re-reads a file when the window is focused and again just before saving. If the file changed outside Yoman while you have unsaved text, Yoman asks you to reload, keep your version, or save a copy.
- Search is a substring match. It is not semantic search.
- Images, task lists, and raw HTML stay read-only so a save cannot drop them.
- The first save may normalize Markdown, such as list markers and emphasis characters.
- Explicit right-to-left or left-to-right blocks are stored as a `<div dir="rtl">` or `<div dir="ltr">` wrapper. Automatic direction is not written into the file.
- Mixed Hebrew and English cursor movement was checked for stored text, paste, and list-marker placement. It was not exercised as a full desktop keyboard pass.
- Delete moves one file to the system trash. It does not permanently delete, and it does not remove directories.
- Colors follow the operating system. There is no separate theme control.
- The application license has not been chosen. Dependency licenses are listed in `docs/decisions.md`.
