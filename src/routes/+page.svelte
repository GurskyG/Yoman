<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import JournalEditor from '#lib/JournalEditor.svelte';
	import {
		chooseJournalFolder,
		createEntry,
		deleteEntry,
		journalStatus,
		readEntry,
		saveEntry
	} from '#lib/journal-client.ts';
	import {
		commandErrorMessage,
		entryPresentation,
		inDesktopApp,
		type JournalSnapshot,
		type OpenedEntry,
		type SaveResponse
	} from '#lib/journal-model.ts';
	import { importMarkdown, exportMarkdown } from '#lib/markdown-roundtrip.ts';
	import {
		collectTags,
		matchesQuery,
		matchesTag,
		normalizeTags,
		sameTags
	} from '#lib/journal-search.ts';

	type SaveResult = 'clean' | 'conflict' | 'error';
	type SaveMode = 'update' | 'overwrite' | 'copy';

	let desktop = $state(false);
	let loading = $state(true);
	let picking = $state(false);
	let opening = $state(false);
	let creating = $state(false);
	let snapshot = $state<JournalSnapshot | null>(null);
	let actionError = $state<string | null>(null);
	let entry = $state<OpenedEntry | null>(null);
	let entryError = $state<string | null>(null);
	let draftTitle = $state('');
	let draftBody = $state('');
	let baselineTitle = $state('');
	let baselineBody = $state('');
	let draftTags = $state<string[]>([]);
	let baselineTags = $state<string[]>([]);
	let tagDraft = $state('');
	let query = $state('');
	let appliedQuery = $state('');
	let tagFilter = $state<string | null>(null);
	let confirmDelete = $state(false);
	let deleting = $state(false);
	let deleteDialog = $state<HTMLDialogElement | null>(null);
	let editable = $state(false);
	let editWarning = $state<string | null>(null);
	let blockedFile = $state<string | null>(null);
	let saveState = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
	let saveMessage = $state('Saved');
	let conflict = $state(false);
	let titleInput = $state<HTMLInputElement | null>(null);
	let debounceTimer: ReturnType<typeof setTimeout> | null = null;
	let saveQueue: Promise<SaveResult> = Promise.resolve('clean');

	const presentation = $derived(entry ? entryPresentation(entry) : null);
	const dirty = $derived(
		draftTitle !== baselineTitle || draftBody !== baselineBody || !sameTags(draftTags, baselineTags)
	);
	const visibleEntries = $derived(
		(snapshot?.entries ?? []).filter(
			(file) => matchesQuery(file, appliedQuery) && matchesTag(file, tagFilter)
		)
	);
	const knownTags = $derived(collectTags(snapshot?.entries ?? []));
	const statusText = $derived.by(() => {
		if (conflict) {
			return saveMessage;
		}
		if (saveState === 'saving') {
			return 'Saving…';
		}
		if (saveState === 'error') {
			return saveMessage;
		}
		if (dirty) {
			return 'Waiting to save…';
		}
		if (saveState === 'saved') {
			return 'Saved';
		}
		return 'Saved';
	});

	onMount(() => {
		let cancelled = false;
		const unlistens: Array<() => void> = [];
		void (async () => {
			await refresh();
			if (cancelled || !inDesktopApp()) {
				return;
			}
			const appWindow = getCurrentWindow();
			unlistens.push(
				await appWindow.onFocusChanged(({ payload: focused }) => {
					if (focused) {
						void recheckFromDisk();
					}
				})
			);
			unlistens.push(
				await appWindow.onCloseRequested(async (event) => {
					if (!dirty && !conflict && saveState !== 'saving') {
						return;
					}
					event.preventDefault();
					const result = await flushPending();
					if (result === 'clean') {
						await appWindow.destroy();
						return;
					}
					if (result === 'conflict') {
						saveMessage =
							'This file changed on disk, so the window stayed open. Reload the file, keep your version, or save a copy.';
					} else if (saveState !== 'error') {
						saveState = 'error';
						saveMessage =
							'Yoman could not save, so the window stayed open. Your text is still in the editor.';
					}
				})
			);
		})();
		return () => {
			cancelled = true;
			clearDebounce();
			for (const unlisten of unlistens) {
				unlisten();
			}
		};
	});

	$effect(() => {
		const nextQuery = query;
		const timer = setTimeout(() => {
			appliedQuery = nextQuery;
		}, 250);
		return () => clearTimeout(timer);
	});

	$effect(() => {
		if (!deleteDialog) {
			return;
		}
		if (confirmDelete && !deleteDialog.open) {
			deleteDialog.showModal();
		}
		if (!confirmDelete && deleteDialog.open) {
			deleteDialog.close();
		}
	});

	$effect(() => {
		const title = draftTitle;
		const body = draftBody;
		const tags = draftTags.join('\n');
		if (!editable || conflict || !dirty || !entry) {
			return;
		}
		const fileName = entry.fileName;
		debounceTimer = setTimeout(() => {
			if (
				entry?.fileName === fileName &&
				draftTitle === title &&
				draftBody === body &&
				draftTags.join('\n') === tags
			) {
				void flushPending();
			}
		}, 700);
		return () => clearDebounce();
	});

	async function refresh() {
		desktop = inDesktopApp();
		if (!desktop) {
			loading = false;
			return;
		}

		loading = true;
		try {
			snapshot = await journalStatus();
		} catch (error) {
			actionError = commandErrorMessage(error);
		} finally {
			loading = false;
		}
	}

	function clearDebounce() {
		if (debounceTimer) {
			clearTimeout(debounceTimer);
			debounceTimer = null;
		}
	}

	function queueSave(mode: SaveMode): Promise<SaveResult> {
		const job = saveQueue.then(() => writeDraft(mode));
		saveQueue = job.then(
			(result) => result,
			() => 'error'
		);
		return job;
	}

	function flushPending(): Promise<SaveResult> {
		clearDebounce();
		return queueSave('update');
	}

	async function writeDraft(mode: SaveMode): Promise<SaveResult> {
		if (!entry || !editable) {
			return dirty ? 'error' : 'clean';
		}
		if (mode === 'update' && conflict) {
			return 'conflict';
		}
		if (mode === 'update' && !dirty) {
			return 'clean';
		}
		const fileName = entry.fileName;
		const baseOriginal = entry.original;
		const title = draftTitle;
		const body = draftBody;
		const tags = draftTags.slice();
		saveState = 'saving';
		saveMessage = 'Saving…';
		try {
			const response = await saveEntry({
				fileName,
				title,
				body,
				baseOriginal,
				mode,
				tags
			});
			if (response.conflict) {
				conflict = true;
				saveState = 'idle';
				saveMessage =
					response.message ?? 'This file changed on disk. Yoman did not overwrite either version.';
				return 'conflict';
			}
			if (!response.saved || !response.entry) {
				saveState = 'error';
				saveMessage =
					response.message ?? 'Yoman could not save the file. Your text is still in the editor.';
				return 'error';
			}
			if (entry?.fileName === fileName || mode === 'copy') {
				acceptSaved(response, mode === 'copy', title, body, tags);
			}
			return conflict ? 'conflict' : 'clean';
		} catch (error) {
			saveState = 'error';
			saveMessage = `${commandErrorMessage(error)} Your text is still in the editor.`;
			return 'error';
		}
	}

	function acceptSaved(
		response: SaveResponse,
		switchFile: boolean,
		savedTitle: string,
		savedBody: string,
		savedTags: string[]
	) {
		const next = response.entry;
		if (!next) {
			return;
		}
		if (response.journal) {
			snapshot = response.journal;
		}
		conflict = false;
		if (switchFile) {
			rememberEntry(next);
		} else {
			if (draftTitle === savedTitle) {
				draftTitle = next.title;
				baselineTitle = next.title;
			} else {
				baselineTitle = savedTitle;
			}
			baselineBody = savedBody;
			if (sameTags(draftTags, savedTags)) {
				draftTags = next.tags;
				baselineTags = next.tags;
			} else {
				baselineTags = savedTags;
			}
			entry = { ...next, title: draftTitle, body: draftBody, tags: draftTags };
		}
		saveState = 'saved';
		saveMessage = 'Saved';
	}

	async function recheckFromDisk() {
		if (!entry || !editable || conflict || opening || creating) {
			return;
		}
		const fileName = entry.fileName;
		const known = entry.original;
		try {
			const next = await readEntry(fileName);
			if (!entry || entry.fileName !== fileName || entry.original !== known) {
				return;
			}
			if (next.original === known) {
				return;
			}
			if (!dirty) {
				rememberEntry(next);
				saveState = 'saved';
				saveMessage = 'Saved';
				return;
			}
			conflict = true;
			saveState = 'idle';
			saveMessage = 'This file changed on disk. Yoman did not overwrite either version.';
		} catch (error) {
			saveState = 'error';
			saveMessage = `${commandErrorMessage(error)} Your text is still in the editor.`;
		}
	}

	async function reloadFromDisk() {
		if (!entry) {
			return;
		}
		const fileName = entry.fileName;
		opening = true;
		try {
			const next = await readEntry(fileName);
			conflict = false;
			rememberEntry(next);
			saveState = 'saved';
			saveMessage = 'Saved';
			blockedFile = null;
		} catch (error) {
			saveState = 'error';
			saveMessage = `${commandErrorMessage(error)} Your text is still in the editor.`;
		} finally {
			opening = false;
		}
	}

	async function keepMyVersion() {
		const result = await queueSave('overwrite');
		if (result === 'clean') {
			blockedFile = null;
		}
	}

	async function saveAsCopy() {
		const result = await queueSave('copy');
		if (result === 'clean') {
			blockedFile = null;
			await tick();
			titleInput?.focus();
		}
	}

	async function createNewEntry() {
		if (!desktop || creating || picking) {
			return;
		}
		const flushed = await flushPending();
		if (flushed !== 'clean') {
			actionError = saveMessage;
			return;
		}
		creating = true;
		actionError = null;
		try {
			const response = await createEntry();
			if (!response.saved || !response.entry) {
				actionError = response.message ?? 'Yoman could not create the entry. Nothing was written.';
				return;
			}
			if (response.journal) {
				snapshot = response.journal;
			}
			blockedFile = null;
			conflict = false;
			rememberEntry(response.entry);
			saveState = 'saved';
			saveMessage = 'Saved';
			await tick();
			titleInput?.focus();
			titleInput?.select();
		} catch (error) {
			actionError = commandErrorMessage(error);
		} finally {
			creating = false;
		}
	}

	async function chooseFolder() {
		if (!desktop || picking) {
			return;
		}
		const flushed = await flushPending();
		if (flushed !== 'clean') {
			actionError = saveMessage;
			return;
		}
		picking = true;
		actionError = null;
		try {
			snapshot = await chooseJournalFolder();
			entry = null;
			entryError = null;
			conflict = false;
			blockedFile = null;
			draftTitle = '';
			draftBody = '';
			baselineTitle = '';
			baselineBody = '';
			draftTags = [];
			baselineTags = [];
			tagDraft = '';
			query = '';
			appliedQuery = '';
			tagFilter = null;
			confirmDelete = false;
			editable = false;
			saveState = 'idle';
		} catch (error) {
			actionError = commandErrorMessage(error);
		} finally {
			picking = false;
		}
	}

	function addTag() {
		const next = normalizeTags([...draftTags, tagDraft]);
		if (next.length !== draftTags.length) {
			draftTags = next;
		}
		tagDraft = '';
	}

	function removeTag(tag: string) {
		draftTags = draftTags.filter((item) => item !== tag);
	}

	function askDelete() {
		if (!entry || deleting) {
			return;
		}
		confirmDelete = true;
	}

	async function moveToTrash() {
		if (!entry || deleting) {
			return;
		}
		const fileName = entry.fileName;
		deleting = true;
		entryError = null;
		try {
			const next = await deleteEntry(fileName);
			snapshot = next;
			if (entry?.fileName === fileName) {
				entry = null;
				draftTitle = '';
				draftBody = '';
				baselineTitle = '';
				baselineBody = '';
				draftTags = [];
				baselineTags = [];
				editable = false;
				conflict = false;
				saveState = 'idle';
			}
			confirmDelete = false;
		} catch (error) {
			entryError = `${commandErrorMessage(error)} The entry is still in the folder.`;
			confirmDelete = false;
		} finally {
			deleting = false;
		}
	}

	function rememberEntry(next: OpenedEntry) {
		entry = next;
		draftTitle = next.title;
		baselineTitle = next.title;
		draftTags = next.tags.slice();
		baselineTags = next.tags.slice();
		tagDraft = '';
		const imported = importMarkdown(next.body);
		if ((presentationOf(next) === 'rendered' || presentationOf(next) === 'empty') && imported.ok) {
			const canonical = exportMarkdown(imported.doc);
			draftBody = canonical;
			baselineBody = canonical;
			editable = true;
			editWarning = null;
			return;
		}
		draftBody = next.body;
		baselineBody = next.body;
		editable = false;
		editWarning = imported.ok ? null : imported.warning;
	}

	function presentationOf(next: OpenedEntry) {
		return entryPresentation(next);
	}

	function formatWhen(value: string | null): string {
		if (!value) {
			return '';
		}
		const parsed = new Date(value);
		if (Number.isNaN(parsed.getTime())) {
			return value;
		}
		return new Intl.DateTimeFormat(undefined, {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(parsed);
	}

	async function openFile(fileName: string) {
		if (!desktop || opening || entry?.fileName === fileName) {
			return;
		}
		const flushed = await flushPending();
		if (flushed !== 'clean') {
			blockedFile = fileName;
			return;
		}
		blockedFile = null;
		await loadFile(fileName);
	}

	async function discardAndOpen() {
		if (!blockedFile) {
			return;
		}
		const fileName = blockedFile;
		blockedFile = null;
		draftTitle = baselineTitle;
		draftBody = baselineBody;
		await loadFile(fileName);
	}

	async function loadFile(fileName: string) {
		opening = true;
		entryError = null;
		try {
			const next = await readEntry(fileName);
			if (!next.readable) {
				entryError = next.warning ?? 'This file could not be read. No files were changed.';
				return;
			}
			rememberEntry(next);
		} catch (error) {
			entryError = commandErrorMessage(error);
		} finally {
			opening = false;
		}
	}
</script>

<main>
	<header class="app-header">
		<p class="brand">Yoman</p>
		<h1>Journal</h1>
		<p class="lede">A private journal stored as Markdown files on this computer.</p>
	</header>

	{#if loading}
		<p role="status">Loading journal…</p>
	{:else if !desktop}
		<section class="panel">
			<h2>Open the desktop app</h2>
			<p>
				Folder selection runs in the Yoman desktop window. This browser preview cannot read a local
				journal folder.
			</p>
		</section>
	{:else}
		{#if !snapshot?.accessible}
			<section class="panel" aria-labelledby="folder-heading">
				<h2 id="folder-heading">Journal folder</h2>
				<p>
					Choose a folder on this computer. Yoman will list the Markdown files saved directly in
					that folder. It does not upload them.
				</p>
				<div class="actions">
					<button type="button" onclick={chooseFolder} disabled={picking || creating}>
						{picking ? 'Waiting for the folder picker…' : 'Choose journal folder'}
					</button>
				</div>
				{#if actionError}
					<p class="alert" role="alert">{actionError}</p>
				{/if}
				{#if snapshot?.error}
					<p class="alert" role="alert">{snapshot.error}</p>
				{/if}
			</section>
		{:else}
			<div class="workspace">
				<aside class="panel sidebar" aria-labelledby="entries-heading">
					<h2 id="entries-heading">Journal</h2>
					<p>
						<strong dir="auto">{snapshot.folderName}</strong>
					</p>
					<p class="meta folder-path" dir="auto">{snapshot.folderPath}</p>
					<div class="sidebar-actions">
						<button type="button" onclick={chooseFolder} disabled={picking || creating}>
							{picking ? 'Waiting for the folder picker…' : 'Change folder'}
						</button>
						<button
							type="button"
							class="primary"
							onclick={createNewEntry}
							disabled={creating || picking}
						>
							{creating ? 'Creating entry…' : 'New entry'}
						</button>
					</div>
					<label class="title-field search-field">
						Search
						<input
							dir="auto"
							bind:value={query}
							type="search"
							autocomplete="off"
							placeholder="Title, text, or tag"
						/>
					</label>
					{#if knownTags.length > 0}
						<div class="tag-row" role="group" aria-label="Filter by tag">
							<button
								type="button"
								class="tag"
								aria-pressed={tagFilter === null}
								onclick={() => (tagFilter = null)}>All</button
							>
							{#each knownTags as tag (tag)}
								<button
									type="button"
									class="tag"
									dir="auto"
									aria-pressed={tagFilter === tag}
									onclick={() => (tagFilter = tagFilter === tag ? null : tag)}>{tag}</button
								>
							{/each}
						</div>
					{/if}
					{#if actionError}
						<p class="alert" role="alert">{actionError}</p>
					{/if}
					{#if snapshot.error}
						<p class="alert" role="alert">{snapshot.error}</p>
					{/if}
					{#if snapshot.entries.length === 0}
						<p>Use <strong>New entry</strong> to create a Markdown file in this folder.</p>
					{:else if visibleEntries.length === 0}
						<p role="status">No entries match this search.</p>
						<button
							type="button"
							onclick={() => {
								query = '';
								appliedQuery = '';
								tagFilter = null;
							}}>Clear search</button
						>
					{:else}
						<p class="count" role="status">
							{visibleEntries.length}
							{visibleEntries.length === 1 ? 'entry' : 'entries'}
						</p>
						<ul class="file-list">
							{#each visibleEntries as file (file.fileName)}
								<li>
									<button
										type="button"
										class="file-button"
										aria-current={entry?.fileName === file.fileName ? 'true' : undefined}
										onclick={() => openFile(file.fileName)}
									>
										<span class="entry-name" dir="auto">{file.title}</span>
										<span class="entry-date" dir="auto"
											>{formatWhen(file.date) || file.fileName}</span
										>
									</button>
								</li>
							{/each}
						</ul>
					{/if}
				</aside>

				<article class="panel editor-panel" aria-labelledby="entry-title">
					{#if opening}
						<p role="status">Opening…</p>
					{/if}
					{#if entryError}
						<p class="alert" role="alert">{entryError}</p>
					{/if}
					{#if blockedFile}
						<p class="alert" role="alert">
							The other file was not opened. Save this entry, or discard the editor text. Discarding
							does not change the file on disk.
						</p>
						<button type="button" onclick={discardAndOpen}>Discard editor text</button>
					{/if}
					{#if entry && presentation !== 'error'}
						<div class="entry-heading">
							<h2 id="entry-title" dir="auto">{draftTitle || entry.fileName}</h2>
							<button type="button" class="danger" onclick={askDelete}>Move to trash</button>
						</div>
						<label class="title-field title-wide">
							Title
							<input
								dir="auto"
								bind:value={draftTitle}
								bind:this={titleInput}
								autocomplete="off"
								disabled={!editable}
							/>
						</label>
						<dl class="meta-grid">
							<div class="wide">
								<dt>File</dt>
								<dd dir="auto">{entry.fileName}</dd>
							</div>
							<div>
								<dt>Status</dt>
								<dd role="status">{editable ? statusText : 'Shown unchanged'}</dd>
							</div>
							{#if entry.date}
								<div>
									<dt>Created</dt>
									<dd dir="auto">{formatWhen(entry.date)}</dd>
								</div>
							{/if}
							{#if entry.updated}
								<div>
									<dt>Updated</dt>
									<dd dir="auto">{formatWhen(entry.updated)}</dd>
								</div>
							{/if}
						</dl>
						{#if conflict}
							<div class="actions">
								<button type="button" onclick={reloadFromDisk}>Reload from disk</button>
								<button type="button" onclick={keepMyVersion}>Keep my version</button>
								<button type="button" onclick={saveAsCopy}>Save my version as a copy</button>
							</div>
						{/if}
						{#if editable}
							<div class="tag-editor">
								<p class="meta">Tags</p>
								<div class="tag-row">
									{#each draftTags as tag (tag)}
										<button type="button" class="tag" dir="auto" onclick={() => removeTag(tag)}>
											{tag} <span aria-hidden="true">×</span>
											<span class="visually-hidden">Remove {tag}</span>
										</button>
									{/each}
								</div>
								<form
									class="tag-form"
									onsubmit={(event) => {
										event.preventDefault();
										addTag();
									}}
								>
									<label class="title-field">
										Add tag
										<input dir="auto" bind:value={tagDraft} autocomplete="off" />
									</label>
									<button type="submit">Add</button>
								</form>
							</div>
						{:else if entry.tags.length > 0}
							<p class="meta" dir="auto">Tags: {entry.tags.join(', ')}</p>
						{/if}
						{#if entry.extra.length > 0}
							<ul class="notice-list">
								{#each entry.extra as field (field.key)}
									<li>
										<strong dir="auto">{field.key}</strong>
										<span dir="auto">{field.value}</span>
									</li>
								{/each}
							</ul>
						{/if}
						{#if presentation === 'original'}
							{#if entry.warning}
								<p class="alert" role="alert">{entry.warning}</p>
							{/if}
							<pre dir="auto">{entry.warning ? entry.original : entry.body}</pre>
						{:else if editable}
							{#if editWarning}
								<p class="alert" role="alert">{editWarning}</p>
							{/if}
							{#key entry.id ?? entry.fileName}
								<JournalEditor
									markdown={baselineBody}
									onChange={(markdown) => (draftBody = markdown)}
									onReady={(markdown) => {
										draftBody = markdown;
										baselineBody = markdown;
									}}
								/>
							{/key}
						{:else}
							{#if editWarning}
								<p class="alert" role="alert">{editWarning}</p>
							{/if}
							<pre dir="auto">{entry.body}</pre>
						{/if}
					{:else if !opening && !entryError}
						<h2 id="entry-title">Entry</h2>
						<p>Select an entry to read and edit it.</p>
					{/if}
				</article>
			</div>
		{/if}

		<dialog
			bind:this={deleteDialog}
			onclose={() => (confirmDelete = false)}
			aria-labelledby="delete-title"
		>
			<h2 id="delete-title">Move this entry to the trash?</h2>
			{#if entry}
				<p dir="auto">{draftTitle || entry.title}</p>
				<p class="meta" dir="auto">{entry.fileName}</p>
			{/if}
			<p>
				Yoman moves this one file to the trash. It does not delete the folder or any other file.
				{#if dirty}
					Unsaved text in the editor will be discarded.
				{/if}
			</p>
			<div class="actions">
				<button type="button" onclick={() => (confirmDelete = false)}>Keep entry</button>
				<button type="button" class="danger" onclick={moveToTrash} disabled={deleting}>
					{deleting ? 'Moving to trash…' : 'Move to trash'}
				</button>
			</div>
		</dialog>

		{#if snapshot && snapshot.notices.length > 0}
			<section class="panel" aria-labelledby="notices-heading">
				<h2 id="notices-heading">Needs attention</h2>
				<ul class="notice-list">
					{#each snapshot.notices as notice (notice.fileName + notice.reason)}
						<li>
							<strong dir="auto">{notice.fileName}</strong>
							<span>{notice.reason}</span>
						</li>
					{/each}
				</ul>
			</section>
		{/if}
	{/if}
</main>
