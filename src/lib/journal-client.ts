import { invoke } from '@tauri-apps/api/core';
import type { JournalSnapshot, OpenedEntry, SaveResponse } from './journal-model';

export function journalStatus(): Promise<JournalSnapshot> {
	return invoke<JournalSnapshot>('journal_status');
}

export function chooseJournalFolder(): Promise<JournalSnapshot> {
	return invoke<JournalSnapshot>('choose_journal_folder');
}

export function readEntry(fileName: string): Promise<OpenedEntry> {
	return invoke<OpenedEntry>('read_entry', { fileName });
}

export function createEntry(): Promise<SaveResponse> {
	return invoke<SaveResponse>('create_entry');
}

export function saveEntry(input: {
	fileName: string;
	title: string;
	body: string;
	baseOriginal: string;
	mode: 'update' | 'overwrite' | 'copy';
	tags: string[];
}): Promise<SaveResponse> {
	return invoke<SaveResponse>('save_entry', input);
}

export function deleteEntry(fileName: string): Promise<JournalSnapshot> {
	return invoke<JournalSnapshot>('delete_entry', { fileName });
}
