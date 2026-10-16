import { describe, expect, it } from 'vitest';
import { exportMarkdown, importMarkdown, plainText, roundTripMarkdown } from './markdown-roundtrip';

const samples = [
	'שלום עולם\n',
	'Hello world\n',
	'שלום from Cursor\n',
	'Today כתבתי TypeScript בקוד\n',
	'המחיר הוא 123.45 ש"ח\n',
	'פתחתי את הקובץ (README.md) בפרויקט\n',
	'https://example.com שלום\n',
	'היום עבדתי על פרויקט חדש ב-Cursor וכתבתי קוד ב-TypeScript.\n',
	'- Cursor\n- TypeScript\n- יומן\n',
	'1. hello עולם\n2. another שורה\n',
	'# Heading\n\n**bold** and *italic* and `code`.\n\n> quoted שלום\n\n---\n\n[example](https://example.com)\n'
];

describe('markdown round trip', () => {
	it.each(samples)('keeps supported markdown stable: %s', (markdown) => {
		const result = roundTripMarkdown(markdown);
		expect(result.ok).toBe(true);
		if (result.ok) {
			expect(result.markdown).toBe(markdown);
		}
	});

	it('keeps Hebrew and English text in logical order', () => {
		const markdown = 'Today כתבתי TypeScript בקוד\n';
		const imported = importMarkdown(markdown);
		expect(imported.ok).toBe(true);
		if (!imported.ok) {
			return;
		}
		expect(plainText(imported.doc)).toBe('Today כתבתי TypeScript בקוד');
		expect(exportMarkdown(imported.doc)).toBe(markdown);
		expect(exportMarkdown(imported.doc).includes('דקוב')).toBe(false);
	});

	it('preserves an explicit direction without reversing the text', () => {
		const markdown = '<div dir="rtl">\n\nשלום from Cursor\n\n</div>\n';
		const imported = importMarkdown(markdown);
		expect(imported.ok).toBe(true);
		if (!imported.ok) {
			return;
		}
		expect(imported.doc.content?.[0]?.attrs?.dir).toBe('rtl');
		expect(plainText(imported.doc)).toBe('שלום from Cursor');
		expect(exportMarkdown(imported.doc)).toBe(markdown);
	});

	it('uses automatic direction when no override is stored', () => {
		const imported = importMarkdown('Hello שלום\n');
		expect(imported.ok).toBe(true);
		if (!imported.ok) {
			return;
		}
		expect(imported.doc.content?.[0]?.attrs?.dir).toBe('auto');
	});

	it('refuses raw HTML and images instead of dropping them', () => {
		const html = importMarkdown('hello\n\n<script>alert(1)</script>\n');
		expect(html.ok).toBe(false);
		if (!html.ok) {
			expect(html.warning).toContain('raw HTML');
		}
		const image = importMarkdown('![alt](https://example.com/a.png)\n');
		expect(image.ok).toBe(false);
	});

	it('does not rewrite an empty body into a placeholder paragraph', () => {
		const imported = importMarkdown('');
		expect(imported.ok).toBe(true);
		if (imported.ok) {
			expect(exportMarkdown(imported.doc)).toBe('');
		}
	});
});
