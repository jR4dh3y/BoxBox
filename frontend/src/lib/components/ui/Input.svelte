<script lang="ts">
	interface Props {
		type?: 'text' | 'password' | 'email' | 'search' | 'number';
		value?: string;
		placeholder?: string;
		disabled?: boolean;
		required?: boolean;
		id?: string;
		name?: string;
		autocomplete?: AutoFill;
		/** Select the base name only, or the whole value, when the field gains focus. */
		selectOnFocus?: 'basename' | 'all';
		oninput?: (e: Event) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}

	let {
		type = 'text',
		value = $bindable(''),
		placeholder,
		disabled = false,
		required = false,
		id,
		name,
		autocomplete,
		selectOnFocus,
		oninput,
		onkeydown
	}: Props = $props();

	function handleFocus(event: FocusEvent & { currentTarget: HTMLInputElement }) {
		if (!selectOnFocus) return;
		const field = event.currentTarget;
		// A leading dot is part of the name (.env), not an extension separator.
		const dot = field.value.lastIndexOf('.');
		field.setSelectionRange(0, selectOnFocus === 'basename' && dot > 0 ? dot : field.value.length);
	}
</script>

<input
	{type}
	bind:value
	{placeholder}
	{disabled}
	{required}
	{id}
	{name}
	{autocomplete}
	{oninput}
	{onkeydown}
	onfocus={handleFocus}
	class="h-8 w-full rounded border border-border-primary bg-surface-primary px-3 text-[13px] text-text-primary transition-colors duration-150 placeholder:text-text-muted focus:border-border-focus focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
/>
