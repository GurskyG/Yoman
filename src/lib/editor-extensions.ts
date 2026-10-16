import Link from '@tiptap/extension-link';
import ListItem from '@tiptap/extension-list-item';
import StarterKit from '@tiptap/starter-kit';
import { Extension } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin } from '@tiptap/pm/state';
import type { EditorView } from '@tiptap/pm/view';

const directionTypes = [
	'paragraph',
	'heading',
	'blockquote',
	'codeBlock',
	'horizontalRule',
	'bulletList',
	'orderedList'
];

export const BlockDirection = Extension.create({
	name: 'blockDirection',
	addGlobalAttributes() {
		return [
			{
				types: directionTypes,
				attributes: {
					dir: {
						default: 'auto',
						parseHTML: (element) => element.getAttribute('dir') ?? 'auto',
						renderHTML: (attributes) => ({ dir: attributes.dir || 'auto' })
					}
				}
			}
		];
	}
});

export function strongDirection(text: string): 'rtl' | 'ltr' | null {
	for (const char of text) {
		const code = char.codePointAt(0) ?? 0;
		if (
			(code >= 0x0590 && code <= 0x08ff) ||
			(code >= 0xfb1d && code <= 0xfdff) ||
			(code >= 0xfe70 && code <= 0xfeff)
		) {
			return 'rtl';
		}
		if (
			(code >= 0x0041 && code <= 0x005a) ||
			(code >= 0x0061 && code <= 0x007a) ||
			(code >= 0x00c0 && code <= 0x024f)
		) {
			return 'ltr';
		}
	}
	return null;
}

export function listItemDirection(doc: ProseMirrorNode, pos: number): 'rtl' | 'ltr' | 'auto' {
	const node = doc.nodeAt(pos);
	if (!node) {
		return 'auto';
	}
	const explicit = node.attrs.dir;
	if (explicit === 'rtl' || explicit === 'ltr') {
		return explicit;
	}
	const own = strongDirection(node.textContent);
	if (own) {
		return own;
	}
	const $pos = doc.resolve(pos);
	const parentDir = $pos.parent.attrs?.dir;
	if (parentDir === 'rtl' || parentDir === 'ltr') {
		return parentDir;
	}
	for (let index = $pos.index() - 1; index >= 0; index -= 1) {
		const found = strongDirection($pos.parent.child(index).textContent);
		if (found) {
			return found;
		}
	}
	if ($pos.depth > 0) {
		const $list = doc.resolve($pos.before($pos.depth));
		for (let index = $list.index() - 1; index >= 0; index -= 1) {
			const found = strongDirection($list.parent.child(index).textContent);
			if (found) {
				return found;
			}
		}
	}
	return 'auto';
}

function syncListMarkerDirection(view: EditorView) {
	let domAt: (pos: number) => globalThis.Node | null;
	try {
		domAt = (pos) => view.nodeDOM(pos);
	} catch {
		return;
	}
	view.state.doc.descendants((node, pos) => {
		if (node.type.name !== 'listItem') {
			return;
		}
		let dom: globalThis.Node | null;
		try {
			dom = domAt(pos);
		} catch {
			return;
		}
		if (!(dom instanceof HTMLElement) || dom.tagName !== 'LI') {
			return;
		}
		const dir = listItemDirection(view.state.doc, pos);
		if (dom.getAttribute('dir') !== dir) {
			dom.setAttribute('dir', dir);
		}
	});
}

const DirectionalListItem = ListItem.extend({
	addAttributes() {
		return {
			...this.parent?.(),
			dir: {
				default: 'auto',
				parseHTML: (element: HTMLElement) => element.getAttribute('dir') ?? 'auto',
				renderHTML: (attributes: { dir?: string }) => ({ dir: attributes.dir || 'auto' })
			}
		};
	},
	addNodeView() {
		return ({ node, HTMLAttributes, getPos, editor }) => {
			const dom = document.createElement('li');
			for (const [key, value] of Object.entries(HTMLAttributes)) {
				if (key === 'dir' || value == null || value === false) {
					continue;
				}
				dom.setAttribute(key, String(value));
			}
			const apply = () => {
				const pos = getPos();
				if (typeof pos !== 'number') {
					return;
				}
				dom.setAttribute('dir', listItemDirection(editor.state.doc, pos));
			};
			apply();
			return {
				dom,
				contentDOM: dom,
				update: (updated) => {
					if (updated.type.name !== node.type.name) {
						return false;
					}
					apply();
					return true;
				},
				ignoreMutation: (mutation) =>
					mutation.type === 'attributes' &&
					mutation.target === dom &&
					mutation.attributeName === 'dir'
			};
		};
	},
	addProseMirrorPlugins() {
		return [
			new Plugin({
				view(editorView) {
					syncListMarkerDirection(editorView);
					return {
						update(view) {
							syncListMarkerDirection(view);
						}
					};
				}
			})
		];
	}
});

export const editorExtensions = [
	StarterKit.configure({
		strike: false,
		underline: false,
		link: false,
		listItem: false
	}),
	DirectionalListItem,
	Link.configure({
		openOnClick: false,
		autolink: false,
		HTMLAttributes: {
			rel: 'noreferrer'
		}
	}),
	BlockDirection
];

export const directionNodeNames = directionTypes.filter((name) => name !== 'horizontalRule');
