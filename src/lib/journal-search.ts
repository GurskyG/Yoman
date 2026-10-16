import type { ListedFile } from './journal-model';

export function normalizeTags(tags: string[]): string[] {
	const normalized: string[] = [];
	for (const tag of tags) {
		const trimmed = tag.trim();
		if (!trimmed || normalized.includes(trimmed)) {
			continue;
		}
		normalized.push(trimmed);
	}
	return normalized;
}

export function sameTags(left: string[], right: string[]): boolean {
	return left.length === right.length && left.every((tag, index) => tag === right[index]);
}

export function matchesQuery(entry: ListedFile, query: string): boolean {
	const needle = query.trim().toLocaleLowerCase();
	if (!needle) {
		return true;
	}
	const haystack = `${entry.title}\n${entry.text}\n${entry.tags.join('\n')}`.toLocaleLowerCase();
	return haystack.includes(needle);
}

export function matchesTag(entry: ListedFile, tag: string | null): boolean {
	if (!tag) {
		return true;
	}
	return entry.tags.includes(tag);
}

export function collectTags(entries: ListedFile[]): string[] {
	const tags = new Set<string>();
	for (const entry of entries) {
		for (const tag of entry.tags) {
			tags.add(tag);
		}
	}
	return [...tags].sort((left, right) => left.localeCompare(right));
}
