<script lang="ts">
	/**
	 * PicoLogo - ASCII art branding component
	 */
	interface Props {
		size?: 'sm' | 'md' | 'lg';
	}

	let { size = 'md' }: Props = $props();

	const sizeClasses = {
		sm: 'text-[8px] leading-[1.1]',
		md: 'text-xs leading-[1.2]',
		lg: 'text-sm leading-[1.2]',
	};
</script>

<pre class="font-mono text-center font-bold whitespace-pre select-none {sizeClasses[size]}">
<span class="text-danger">██████╗</span> <span class="text-warning">██╗</span> <span class="text-success">██████╗</span> <span class="text-accent">██████╗</span>
<span class="text-danger"> ██╔══██╗</span><span class="text-warning">██║</span><span class="text-success">██╔════╝</span><span class="text-accent">██╔═══██╗</span>
<span class="text-danger"> ██████╔╝</span><span class="text-warning">██║</span><span class="text-success">██║     </span><span class="text-accent">██║   ██║</span>
<span class="text-danger"> ██╔═══╝ </span><span class="text-warning">██║</span><span class="text-success">██║     </span><span class="text-accent">██║   ██║</span>
<span class="text-danger"> ██║     </span><span class="text-warning">██║</span><span class="text-success">╚██████╗</span><span class="text-accent">╚██████╔╝</span>
<span class="text-danger">╚═╝     </span><span class="text-warning">╚═╝</span> <span class="text-success">╚═════╝</span> <span class="text-accent">╚═════╝</span>
</pre>
