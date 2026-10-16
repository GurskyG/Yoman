export type ListedFile = {
	fileName: string;
	title: string;
	date: string | null;
	tags: string[];
	text: string;
};

export type FileNotice = {
	fileName: string;
	reason: string;
};

export type JournalSnapshot = {
	folderName: string | null;
	folderPath: string | null;
	accessible: boolean;
	entries: ListedFile[];
	notices: FileNotice[];
	error: string | null;
};

export type FrontmatterField = {
	key: string;
	value: string;
};

export type Inline =
	| { kind: 'text'; text: string }
	| { kind: 'strong'; inlines: Inline[] }
	| { kind: 'emphasis'; inlines: Inline[] }
	| { kind: 'code'; text: string }
	| { kind: 'link'; href: string; inlines: Inline[] }
	| { kind: 'break'; hard: boolean };

export type Block =
	| { kind: 'heading'; level: number; inlines: Inline[] }
	| { kind: 'paragraph'; inlines: Inline[] }
	| { kind: 'list'; ordered: boolean; items: Block[][] }
	| { kind: 'quote'; blocks: Block[] }
	| { kind: 'code'; text: string }
	| { kind: 'rule' };

export type SaveResponse = {
	saved: boolean;
	conflict: boolean;
	entry: OpenedEntry | null;
	diskOriginal: string | null;
	journal: JournalSnapshot | null;
	message: string | null;
};

export type OpenedEntry = {
	fileName: string;
	readable: boolean;
	title: string;
	body: string;
	original: string;
	warning: string | null;
	date: string | null;
	updated: string | null;
	id: string | null;
	tags: string[];
	extra: FrontmatterField[];
	blocks: Block[];
};

export type EntryPresentation = 'error' | 'original' | 'rendered' | 'empty';

export function entryPresentation(entry: OpenedEntry): EntryPresentation {
	if (!entry.readable) {
		return 'error';
	}
	if (entry.warning || entry.blocks.length === 0) {
		return entry.body.trim() ? 'original' : 'empty';
	}
	return 'rendered';
}

export type JournalScreen = 'unconfigured' | 'unavailable' | 'empty' | 'ready';

export function journalScreen(snapshot: JournalSnapshot): JournalScreen {
	if (!snapshot.folderPath) {
		return snapshot.error ? 'unavailable' : 'unconfigured';
	}
	if (!snapshot.accessible) {
		return 'unavailable';
	}
	if (snapshot.entries.length === 0) {
		return 'empty';
	}
	return 'ready';
}

export function inDesktopApp(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export function commandErrorMessage(error: unknown): string {
	if (typeof error === 'string' && error.trim()) {
		return error;
	}
	if (error instanceof Error && error.message.trim()) {
		return error.message;
	}
	return 'Something went wrong. Your journal files were not changed.';
}
