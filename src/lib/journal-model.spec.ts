import { describe, expect, it } from 'vitest';
import {
	commandErrorMessage,
	entryPresentation,
	inDesktopApp,
	journalScreen,
	type JournalSnapshot,
	type OpenedEntry
} from './journal-model';

function snapshot(overrides: Partial<JournalSnapshot> = {}): JournalSnapshot {
	return {
		folderName: null,
		folderPath: null,
		accessible: false,
		entries: [],
		notices: [],
		error: null,
		...overrides
	};
}

describe('journalScreen', () => {
	it('asks for a folder when none is configured', () => {
		expect(journalScreen(snapshot())).toBe('unconfigured');
	});

	it('shows an error when settings cannot be read', () => {
		expect(journalScreen(snapshot({ error: 'settings unreadable' }))).toBe('unavailable');
	});

	it('shows an error when the saved folder cannot be read', () => {
		expect(
			journalScreen(
				snapshot({
					folderName: 'journal',
					folderPath: '/tmp/journal',
					accessible: false,
					error: 'missing'
				})
			)
		).toBe('unavailable');
	});

	it('explains an empty folder separately from a folder with files', () => {
		expect(
			journalScreen(
				snapshot({
					folderName: 'journal',
					folderPath: '/tmp/journal',
					accessible: true
				})
			)
		).toBe('empty');
		expect(
			journalScreen(
				snapshot({
					folderName: 'journal',
					folderPath: '/tmp/journal',
					accessible: true,
					entries: [
						{
							fileName: 'שלום.md',
							title: 'שלום',
							date: null,
							tags: [],
							text: ''
						}
					]
				})
			)
		).toBe('ready');
	});
});

describe('desktop and command errors', () => {
	it('does not treat a plain browser window as the desktop app', () => {
		expect(inDesktopApp()).toBe(false);
	});

	it('recognizes the Tauri runtime marker', () => {
		const previous = globalThis.window;
		globalThis.window = { __TAURI_INTERNALS__: {} } as unknown as Window & typeof globalThis.window;
		expect(inDesktopApp()).toBe(true);
		globalThis.window = previous;
	});

	it('keeps string errors from the native command', () => {
		expect(commandErrorMessage('Could not open that folder.')).toBe('Could not open that folder.');
		expect(commandErrorMessage(new Error('disk full'))).toBe('disk full');
		expect(commandErrorMessage(null)).toBe(
			'Something went wrong. Your journal files were not changed.'
		);
	});
});

function opened(overrides: Partial<OpenedEntry> = {}): OpenedEntry {
	return {
		fileName: 'note.md',
		readable: true,
		title: 'Note',
		body: 'hello',
		original: 'hello',
		warning: null,
		date: null,
		updated: null,
		id: null,
		tags: [],
		extra: [],
		blocks: [{ kind: 'paragraph', inlines: [{ kind: 'text', text: 'hello' }] }],
		...overrides
	};
}

describe('entryPresentation', () => {
	it('does not treat an unreadable file as an empty entry', () => {
		expect(entryPresentation(opened({ readable: false, body: '', blocks: [] }))).toBe('error');
	});

	it('shows the original text when frontmatter cannot be parsed', () => {
		expect(entryPresentation(opened({ warning: 'broken', blocks: [] }))).toBe('original');
	});

	it('shows a rendered entry when blocks exist', () => {
		expect(entryPresentation(opened())).toBe('rendered');
	});

	it('shows an empty state only when the body is blank', () => {
		expect(entryPresentation(opened({ body: '  \n', blocks: [] }))).toBe('empty');
	});
});
