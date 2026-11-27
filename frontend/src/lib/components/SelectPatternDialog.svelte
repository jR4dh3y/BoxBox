<script lang="ts">
	/** Select by Pattern: picks the items a pattern such as `*.jpg` matches. */
	import { untrack } from 'svelte';
	import { Button, Input, Modal } from '#lib/components/ui/index.js';

	let {
		initial,
		onselect,
		onclose
	}: {
		initial: string;
		/** Selects what the pattern matches; returns how many items that was. */
		onselect: (pattern: string) => number;
		onclose: () => void;
	} = $props();

	// The field starts with the last pattern and then belongs to the user.
	let pattern = $state(untrack(() => initial));
	let error = $state<string | null>(null);

	function select(): void {
		const value = pattern.trim();
		// A pattern that matches nothing keeps the dialog open to be changed.
		if (!value) error = 'Type a pattern, such as *.jpg';
		else if (onselect(value) === 0) error = `Nothing here matches “${value}”`;
		else onclose();
	}
</script>

<Modal open title="Select by Pattern" {onclose}>
	<form
		class="flex flex-col gap-2"
		onsubmit={(event) => {
			event.preventDefault();
			select();
		}}
	>
		<p class="m-0 text-[13px] text-text-secondary">
			Use * for any characters and ? for one, as in *.jpg; plain text matches anywhere in a name.
		</p>
		<Input bind:value={pattern} placeholder="*.jpg" selectOnFocus="all" />
		{#if error}<p class="m-0 text-[13px] text-danger" role="alert">{error}</p>{/if}
	</form>
	{#snippet footer()}
		<Button variant="secondary" onclick={onclose}>Cancel</Button>
		<Button onclick={select}>Select</Button>
	{/snippet}
</Modal>
