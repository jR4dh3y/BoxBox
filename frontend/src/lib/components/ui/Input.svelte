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
		oninput,
		onkeydown
	}: Props = $props();
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
	class="w-full rounded border border-border-primary bg-surface-secondary px-3 py-2 text-sm text-text-primary transition-colors duration-150 placeholder:text-text-muted focus:border-border-focus focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
/>
