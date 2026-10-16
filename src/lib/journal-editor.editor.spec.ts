import { Editor } from '@tiptap/core';
import { describe, expect, it } from 'vitest';
import { editorExtensions } from './editor-extensions';
import { exportMarkdown, importMarkdown, plainText } from './markdown-roundtrip';

function editorFor(markdown: string): Editor {
	const imported = importMarkdown(markdown);
	if (!imported.ok) {
		throw new Error(imported.warning);
	}
	return new Editor({
		element: document.createElement('div'),
		extensions: editorExtensions,
		content: imported.doc
	});
}

describe('journal editor', () => {
	it('keeps mixed Hebrew and English text in logical order', () => {
		const editor = editorFor('שלום from Cursor\n');
		expect(editor.getText()).toBe('שלום from Cursor');
		expect(plainText(editor.getJSON())).toBe('שלום from Cursor');
		editor.destroy();
	});

	it('inserts pasted mixed text without reversing it', () => {
		const editor = editorFor('');
		editor.commands.insertContent('המחיר הוא 123.45 ש"ח');
		expect(editor.getText()).toBe('המחיר הוא 123.45 ש"ח');
		editor.destroy();
	});

	it('puts the bullet on the right for Hebrew list items', () => {
		const editor = editorFor('- הפל הוצג\n');
		const item = editor.view.dom.querySelector('li');
		expect(item?.getAttribute('dir')).toBe('rtl');
		expect(exportMarkdown(editor.getJSON())).not.toContain('<div dir=');
		editor.destroy();
	});

	it('keeps a new bullet with the Hebrew paragraph above it', () => {
		const editor = editorFor('הניסיון השני\n');
		editor.commands.focus('end');
		editor.commands.splitBlock();
		editor.commands.toggleBulletList();
		const item = editor.view.dom.querySelector('li');
		expect(item?.getAttribute('dir')).toBe('rtl');
		editor.destroy();
	});

	it('round-trips bold, lists, and an explicit direction', () => {
		const editor = editorFor('- English\n- עברית\n');
		let range: { from: number; to: number } | null = null;
		editor.state.doc.descendants((node, pos) => {
			if (!node.isText || !node.text?.includes('English')) {
				return;
			}
			const start = node.text.indexOf('English');
			range = { from: pos + start, to: pos + start + 'English'.length };
		});
		expect(range).not.toBeNull();
		if (range) {
			editor.commands.setTextSelection(range);
		}
		editor.commands.toggleBold();
		editor.commands.updateAttributes('bulletList', { dir: 'rtl' });
		const markdown = exportMarkdown(editor.getJSON());
		expect(markdown).toContain('**English**');
		expect(markdown).toContain('עברית');
		expect(markdown).toContain('<div dir="rtl">');
		const imported = importMarkdown(markdown);
		expect(imported.ok).toBe(true);
		if (imported.ok) {
			expect(plainText(imported.doc)).toContain('English');
			expect(plainText(imported.doc)).toContain('עברית');
			expect(imported.doc.content?.[0]?.attrs?.dir).toBe('rtl');
		}
		editor.destroy();
	});
});
