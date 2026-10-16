<script lang="ts">
	import { Editor } from '@tiptap/core';
	import { onDestroy, onMount } from 'svelte';
	import { directionNodeNames, editorExtensions } from '#lib/editor-extensions.ts';
	import { exportMarkdown, importMarkdown, type BlockDirection } from '#lib/markdown-roundtrip.ts';

	let {
		markdown,
		onChange,
		onReady
	}: {
		markdown: string;
		onChange: (markdown: string) => void;
		onReady: (markdown: string) => void;
	} = $props();

	let host: HTMLDivElement | undefined = $state();
	let linkHref = $state('');
	let editor = $state<Editor | null>(null);
	let active = $state({
		bold: false,
		italic: false,
		bullet: false,
		ordered: false,
		quote: false,
		dir: 'auto' as BlockDirection
	});

	onMount(() => {
		const imported = importMarkdown(markdown);
		if (!imported.ok || !host) {
			return;
		}
		const instance = new Editor({
			element: host,
			extensions: editorExtensions,
			content: imported.doc,
			editorProps: {
				attributes: {
					class: 'journal-editor-body',
					'aria-label': 'Journal body',
					dir: 'auto'
				}
			},
			onCreate: ({ editor: created }) => {
				onReady(exportMarkdown(created.getJSON()));
				refresh(created);
			},
			onUpdate: ({ editor: updated }) => {
				onChange(exportMarkdown(updated.getJSON()));
				refresh(updated);
			},
			onSelectionUpdate: ({ editor: updated }) => {
				refresh(updated);
			}
		});
		editor = instance;
	});

	onDestroy(() => {
		editor?.destroy();
	});

	function refresh(instance: Editor) {
		active = {
			bold: instance.isActive('bold'),
			italic: instance.isActive('italic'),
			bullet: instance.isActive('bulletList'),
			ordered: instance.isActive('orderedList'),
			quote: instance.isActive('blockquote'),
			dir: currentDirection(instance)
		};
	}

	function currentDirection(instance: Editor): BlockDirection {
		for (const name of directionNodeNames) {
			if (instance.isActive(name)) {
				const dir = instance.getAttributes(name).dir;
				if (dir === 'rtl' || dir === 'ltr' || dir === 'auto') {
					return dir;
				}
			}
		}
		return 'auto';
	}

	function run(command: (instance: Editor) => void) {
		if (!editor) {
			return;
		}
		command(editor);
		editor.commands.focus();
	}

	function setDirection(dir: BlockDirection) {
		run((instance) => {
			for (const name of directionNodeNames) {
				if (instance.isActive(name)) {
					instance.commands.updateAttributes(name, { dir });
					return;
				}
			}
		});
	}

	function applyLink() {
		const href = linkHref.trim();
		run((instance) => {
			if (!href) {
				instance.commands.unsetLink();
				return;
			}
			instance.commands.setLink({ href });
		});
	}
</script>

<div class="journal-editor">
	<div class="toolbar" role="toolbar" aria-label="Formatting">
		<button
			type="button"
			aria-pressed={active.bold}
			onclick={() => run((instance) => instance.chain().focus().toggleBold().run())}>Bold</button
		>
		<button
			type="button"
			aria-pressed={active.italic}
			onclick={() => run((instance) => instance.chain().focus().toggleItalic().run())}
			>Italic</button
		>
		<button
			type="button"
			onclick={() => run((instance) => instance.chain().focus().toggleHeading({ level: 2 }).run())}
			>Heading</button
		>
		<button
			type="button"
			aria-pressed={active.bullet}
			onclick={() => run((instance) => instance.chain().focus().toggleBulletList().run())}
			>Bullets</button
		>
		<button
			type="button"
			aria-pressed={active.ordered}
			onclick={() => run((instance) => instance.chain().focus().toggleOrderedList().run())}
			>Numbers</button
		>
		<button
			type="button"
			aria-pressed={active.quote}
			onclick={() => run((instance) => instance.chain().focus().toggleBlockquote().run())}
			>Quote</button
		>
		<button
			type="button"
			onclick={() => run((instance) => instance.chain().focus().setHorizontalRule().run())}
			>Rule</button
		>
		<button type="button" onclick={() => run((instance) => instance.chain().focus().undo().run())}
			>Undo</button
		>
		<button type="button" onclick={() => run((instance) => instance.chain().focus().redo().run())}
			>Redo</button
		>
		<button type="button" aria-pressed={active.dir === 'auto'} onclick={() => setDirection('auto')}
			>Auto direction</button
		>
		<button type="button" aria-pressed={active.dir === 'rtl'} onclick={() => setDirection('rtl')}
			>Right to left</button
		>
		<button type="button" aria-pressed={active.dir === 'ltr'} onclick={() => setDirection('ltr')}
			>Left to right</button
		>
	</div>
	<form
		class="link-row"
		onsubmit={(event) => {
			event.preventDefault();
			applyLink();
		}}
	>
		<label>
			Link address
			<input bind:value={linkHref} type="text" dir="auto" autocomplete="off" />
		</label>
		<button type="submit">Apply link</button>
	</form>
	<p class="hint">Direction follows the text until you set the current block explicitly.</p>
	<div bind:this={host}></div>
</div>
