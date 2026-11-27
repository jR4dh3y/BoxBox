<script lang="ts">
	/** Renames an item where its name is drawn: Enter keeps the name, Escape puts it back. */
	import { untrack } from 'svelte';
	import { basenameEnd } from '#lib/browse/names.js';

	let {
		value,
		isDir,
		problem,
		align = 'left',
		oninput,
		oncommit,
		oncancel
	}: {
		value: string;
		isDir: boolean;
		/** Why the name cannot be used, or null. */
		problem: string | null;
		align?: 'left' | 'center';
		oninput: (name: string) => void;
		oncommit: () => void;
		oncancel: () => void;
	} = $props();

	/** Once, as the field appears: the name before its extension, so typing replaces only that. */
	function selectBasename(input: HTMLInputElement): void {
		untrack(() => {
			input.focus();
			input.setSelectionRange(0, basenameEnd(input.value, isDir));
		});
	}

	let done = false;
	function finish(commit: boolean): void {
		if (done) return;
		done = true;
		if (commit && problem === null) oncommit();
		else oncancel();
	}
</script>

<span class="relative block w-full min-w-0">
	<input
		class="box-border h-6 w-full min-w-0 rounded border bg-surface-primary px-1.5 text-[13px] text-text-primary outline-none {problem
			? 'border-danger'
			: 'border-border-focus'} {align === 'center' ? 'text-center' : ''}"
		{value}
		aria-label="New name"
		aria-invalid={problem !== null}
		spellcheck="false"
		{@attach selectBasename}
		oninput={(event) => oninput(event.currentTarget.value)}
		onkeydown={(event) => {
			// The list's own keys stay out of the field.
			event.stopPropagation();
			if (event.key === 'Enter') {
				event.preventDefault();
				if (problem === null) finish(true);
			} else if (event.key === 'Escape') {
				event.preventDefault();
				finish(false);
			}
		}}
		onblur={() => finish(true)}
		onclick={(event) => event.stopPropagation()}
		ondblclick={(event) => event.stopPropagation()}
		onpointerdown={(event) => event.stopPropagation()}
	/>
	{#if problem}
		<span
			class="absolute top-full left-0 z-20 mt-1 rounded border border-danger/60 bg-surface-elevated px-2 py-1 text-xs whitespace-nowrap text-danger shadow-lg"
			role="alert">{problem}</span
		>
	{/if}
</span>
