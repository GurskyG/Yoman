import { describe, expect, it } from 'vitest';
import type { ListedFile } from './journal-model';
import { collectTags, matchesQuery, matchesTag, normalizeTags } from './journal-search';

function entry(overrides: Partial<ListedFile> = {}): ListedFile {
	return {
		fileName: '2026-10-16-100000-note.md',
		title: 'Hello',
		date: '2026-10-16T10:00:00Z',
		tags: ['Work'],
		text: 'Today I wrote TypeScript',
		...overrides
	};
}

describe('journal search', () => {
	it('matches Hebrew text and ignores Latin case', () => {
		const hebrew = entry({ title: 'יום', text: 'שלום עולם', tags: ['אישי'] });
		expect(matchesQuery(hebrew, 'שלום')).toBe(true);
		expect(matchesQuery(entry(), 'typescript')).toBe(true);
		expect(matchesQuery(entry(), 'missing')).toBe(false);
	});

	it('filters by an exact tag and keeps spelling', () => {
		const tags = normalizeTags(['  אישי  ', 'אישי', 'Work', '']);
		expect(tags).toEqual(['אישי', 'Work']);
		expect(matchesTag(entry({ tags }), 'אישי')).toBe(true);
		expect(matchesTag(entry({ tags }), 'work')).toBe(false);
		const collected = collectTags([entry({ tags }), entry({ tags: ['Work', 'Home'] })]);
		expect(collected).toHaveLength(3);
		expect(collected).toContain('אישי');
		expect(collected).toContain('Work');
		expect(collected).toContain('Home');
	});
});
