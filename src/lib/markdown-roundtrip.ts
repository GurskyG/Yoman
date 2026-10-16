import { fromMarkdown } from 'mdast-util-from-markdown';
import { toMarkdown } from 'mdast-util-to-markdown';

export type BlockDirection = 'auto' | 'rtl' | 'ltr';

export type MarkdownMark = {
	type: 'bold' | 'italic' | 'code' | 'link';
	attrs?: { href?: string; title?: string | null };
};

export type MarkdownNode = {
	type: string;
	attrs?: Record<string, unknown>;
	text?: string;
	marks?: MarkdownMark[];
	content?: MarkdownNode[];
};

type MdNode = {
	type: string;
	value?: string;
	children?: MdNode[];
	depth?: 1 | 2 | 3 | 4 | 5 | 6;
	ordered?: boolean;
	start?: number | null;
	checked?: boolean | null;
	spread?: boolean;
	url?: string;
	title?: string | null;
	lang?: string | null;
};

const markdownOptions = {
	bullet: '-' as const,
	rule: '-' as const,
	emphasis: '*' as const,
	strong: '*' as const
};

const supportedTypes = new Set([
	'root',
	'paragraph',
	'heading',
	'text',
	'strong',
	'emphasis',
	'inlineCode',
	'link',
	'blockquote',
	'list',
	'listItem',
	'code',
	'thematicBreak',
	'break',
	'html'
]);

export type ImportResult = { ok: true; doc: MarkdownNode } | { ok: false; warning: string };

export function importMarkdown(markdown: string): ImportResult {
	const tree = fromMarkdown(markdown) as MdNode;
	const unsupported = findUnsupported(tree);
	if (unsupported) {
		return {
			ok: false,
			warning: `This file contains ${unsupported}. Yoman is showing the original text unchanged so none of it is dropped.`
		};
	}
	return {
		ok: true,
		doc: {
			type: 'doc',
			content: convertFlow(tree.children ?? [], 'auto')
		}
	};
}

export function exportMarkdown(doc: MarkdownNode): string {
	if (isEmptyDocument(doc)) {
		return '';
	}
	const tree: MdNode = {
		type: 'root',
		children: exportFlow(doc.content ?? [])
	};
	return toMarkdown(tree as never, markdownOptions);
}

export function roundTripMarkdown(markdown: string): ImportResult & { markdown?: string } {
	const imported = importMarkdown(markdown);
	if (!imported.ok) {
		return imported;
	}
	return { ok: true, doc: imported.doc, markdown: exportMarkdown(imported.doc) };
}

function findUnsupported(node: MdNode): string | null {
	if (node.type === 'html') {
		if (directionOpen(node.value ?? '') || directionClose(node.value ?? '')) {
			return null;
		}
		return 'raw HTML';
	}
	if (node.type === 'listItem' && node.checked != null) {
		return 'a task list';
	}
	if (!supportedTypes.has(node.type)) {
		return node.type;
	}
	for (const child of node.children ?? []) {
		const found = findUnsupported(child);
		if (found) {
			return found;
		}
	}
	return null;
}

function convertFlow(nodes: MdNode[], inherited: BlockDirection): MarkdownNode[] {
	const blocks: MarkdownNode[] = [];
	let forced: BlockDirection | null = null;
	for (const node of nodes) {
		const opening = directionOpen(node.value ?? '');
		if (node.type === 'html' && opening) {
			forced = opening;
			continue;
		}
		if (node.type === 'html' && directionClose(node.value ?? '')) {
			forced = null;
			continue;
		}
		const block = convertBlock(node, forced ?? inherited);
		if (block) {
			blocks.push(block);
		}
	}
	return blocks;
}

function convertBlock(node: MdNode, dir: BlockDirection): MarkdownNode | null {
	if (node.type === 'paragraph') {
		return {
			type: 'paragraph',
			attrs: { dir },
			content: convertPhrasing(node.children ?? [])
		};
	}
	if (node.type === 'heading') {
		return {
			type: 'heading',
			attrs: { level: node.depth ?? 1, dir },
			content: convertPhrasing(node.children ?? [])
		};
	}
	if (node.type === 'blockquote') {
		return {
			type: 'blockquote',
			attrs: { dir },
			content: convertFlow(node.children ?? [], 'auto')
		};
	}
	if (node.type === 'list') {
		return {
			type: node.ordered ? 'orderedList' : 'bulletList',
			attrs: node.ordered ? { dir, start: node.start ?? 1 } : { dir },
			content: (node.children ?? []).map((item) => ({
				type: 'listItem',
				content: convertFlow(item.children ?? [], 'auto')
			}))
		};
	}
	if (node.type === 'code') {
		const content = node.value ? [{ type: 'text', text: node.value }] : [];
		return {
			type: 'codeBlock',
			attrs: { dir, language: node.lang ?? null },
			content
		};
	}
	if (node.type === 'thematicBreak') {
		return { type: 'horizontalRule', attrs: { dir } };
	}
	return null;
}

function convertPhrasing(nodes: MdNode[]): MarkdownNode[] {
	const content: MarkdownNode[] = [];
	for (const node of nodes) {
		if (node.type === 'text') {
			if (node.value) {
				content.push({ type: 'text', text: node.value });
			}
			continue;
		}
		if (node.type === 'break') {
			content.push({ type: 'hardBreak' });
			continue;
		}
		if (node.type === 'inlineCode') {
			content.push({
				type: 'text',
				text: node.value ?? '',
				marks: [{ type: 'code' }]
			});
			continue;
		}
		if (node.type === 'strong' || node.type === 'emphasis' || node.type === 'link') {
			const mark: MarkdownMark =
				node.type === 'strong'
					? { type: 'bold' }
					: node.type === 'emphasis'
						? { type: 'italic' }
						: {
								type: 'link',
								attrs: { href: node.url ?? '', title: node.title ?? null }
							};
			for (const child of convertPhrasing(node.children ?? [])) {
				content.push(addMark(child, mark));
			}
		}
	}
	return content;
}

function addMark(node: MarkdownNode, mark: MarkdownMark): MarkdownNode {
	if (node.type !== 'text') {
		return node;
	}
	return { ...node, marks: [...(node.marks ?? []), mark] };
}

function exportFlow(nodes: MarkdownNode[]): MdNode[] {
	const children: MdNode[] = [];
	for (const node of nodes) {
		children.push(...exportBlock(node));
	}
	return children;
}

function exportBlock(node: MarkdownNode): MdNode[] {
	const dir = directionOf(node);
	let blocks: MdNode[];
	if (node.type === 'paragraph') {
		blocks = [{ type: 'paragraph', children: exportPhrasing(node.content ?? []) }];
	} else if (node.type === 'heading') {
		blocks = [
			{
				type: 'heading',
				depth: headingDepth(node.attrs?.level),
				children: exportPhrasing(node.content ?? [])
			}
		];
	} else if (node.type === 'blockquote') {
		blocks = [{ type: 'blockquote', children: exportFlow(node.content ?? []) }];
	} else if (node.type === 'bulletList' || node.type === 'orderedList') {
		const list: MdNode = {
			type: 'list',
			ordered: node.type === 'orderedList',
			start: node.type === 'orderedList' ? numberAttr(node.attrs?.start, 1) : null,
			spread: false,
			children: (node.content ?? []).map((item) => ({
				type: 'listItem',
				checked: null,
				spread: false,
				children: exportFlow(item.content ?? [])
			}))
		};
		blocks = [list];
	} else if (node.type === 'codeBlock') {
		blocks = [
			{
				type: 'code',
				lang: typeof node.attrs?.language === 'string' ? node.attrs.language : null,
				value: (node.content ?? []).map((child) => child.text ?? '').join('')
			}
		];
	} else if (node.type === 'horizontalRule') {
		blocks = [{ type: 'thematicBreak' }];
	} else if (node.type === 'listItem') {
		blocks = [{ type: 'listItem', checked: null, children: exportFlow(node.content ?? []) }];
	} else {
		blocks = [];
	}
	if ((dir === 'rtl' || dir === 'ltr') && blocks.length === 1) {
		return [
			{ type: 'html', value: `<div dir="${dir}">` },
			blocks[0],
			{ type: 'html', value: '</div>' }
		];
	}
	return blocks;
}

function exportPhrasing(nodes: MarkdownNode[]): MdNode[] {
	const children: MdNode[] = [];
	for (const node of nodes) {
		if (node.type === 'hardBreak') {
			children.push({ type: 'break' });
			continue;
		}
		if (node.type !== 'text' || !node.text) {
			continue;
		}
		children.push(textWithMarks(node.text, node.marks ?? []));
	}
	return children;
}

function textWithMarks(text: string, marks: MarkdownMark[]): MdNode {
	if (marks.some((mark) => mark.type === 'code')) {
		return { type: 'inlineCode', value: text };
	}
	let current: MdNode = { type: 'text', value: text };
	if (marks.some((mark) => mark.type === 'italic')) {
		current = { type: 'emphasis', children: [current] };
	}
	if (marks.some((mark) => mark.type === 'bold')) {
		current = { type: 'strong', children: [current] };
	}
	const link = marks.find((mark) => mark.type === 'link');
	if (link) {
		current = {
			type: 'link',
			url: link.attrs?.href ?? '',
			title: link.attrs?.title ?? null,
			children: [current]
		};
	}
	return current;
}

function directionOf(node: MarkdownNode): BlockDirection {
	const dir = node.attrs?.dir;
	if (dir === 'rtl' || dir === 'ltr' || dir === 'auto') {
		return dir;
	}
	return 'auto';
}

function headingDepth(level: unknown): 1 | 2 | 3 | 4 | 5 | 6 {
	const value = typeof level === 'number' ? level : 1;
	if (value < 1) return 1;
	if (value > 6) return 6;
	return value as 1 | 2 | 3 | 4 | 5 | 6;
}

function numberAttr(value: unknown, fallback: number): number {
	return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function isEmptyDocument(doc: MarkdownNode): boolean {
	const content = doc.content ?? [];
	if (content.length === 0) {
		return true;
	}
	if (content.length !== 1 || content[0]?.type !== 'paragraph') {
		return false;
	}
	const paragraph = content[0];
	const dir = directionOf(paragraph);
	const text = (paragraph.content ?? []).every((node) => node.type === 'text' && !node.text);
	return dir === 'auto' && ((paragraph.content ?? []).length === 0 || text);
}

function directionOpen(value: string): BlockDirection | null {
	const match = value.trim().match(/^<div dir="(rtl|ltr)">$/);
	if (match?.[1] === 'rtl' || match?.[1] === 'ltr') {
		return match[1];
	}
	return null;
}

function directionClose(value: string): boolean {
	return value.trim() === '</div>';
}

export function plainText(doc: MarkdownNode): string {
	const parts: string[] = [];
	collectText(doc, parts);
	return parts.join('');
}

function collectText(node: MarkdownNode, parts: string[]) {
	if (node.text) {
		parts.push(node.text);
	}
	for (const child of node.content ?? []) {
		collectText(child, parts);
	}
}
