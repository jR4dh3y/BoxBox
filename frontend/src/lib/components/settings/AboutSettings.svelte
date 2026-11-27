<script lang="ts">
	/** Settings → About: what BoxBox is, and the project on GitHub, as the desktop app shows it. */
	import { Bug, Heart, Star } from 'lucide-svelte';

	const REPOSITORY = 'https://github.com/jR4dh3y/BoxBox';
	const SPONSOR = 'https://github.com/sponsors/jR4dh3y';
	const ISSUES = 'https://github.com/jR4dh3y/BoxBox/issues';

	const linkClass =
		'inline-flex items-center gap-1.5 rounded border px-3 py-1.5 text-[13px] no-underline transition-colors';
</script>

<div class="flex min-h-full flex-col gap-6 py-4">
	<div class="flex items-center gap-4">
		<img src="/logo-boxbox-3d.svg" alt="" class="h-[72px] w-[72px] shrink-0" />
		<div class="flex min-w-0 flex-col gap-1">
			<h3 class="m-0 text-lg font-semibold text-text-primary">BoxBox</h3>
			<p class="m-0 text-[13px] text-text-secondary">A self-hosted file manager</p>
		</div>
	</div>

	<div class="mt-auto flex flex-col items-center gap-2.5">
		<p class="m-0 text-center text-xs text-text-secondary">
			Free and open source. A star or a sponsorship keeps it going.
		</p>
		<div class="flex flex-wrap justify-center gap-2">
			<a
				class="{linkClass} border-border-primary text-text-primary hover:bg-surface-elevated"
				href={REPOSITORY}
				target="_blank"
				rel="noopener noreferrer"><Star size={14} />Star on GitHub</a
			>
			<a
				class="{linkClass} border-accent bg-accent text-white hover:opacity-90"
				href={SPONSOR}
				target="_blank"
				rel="noopener noreferrer"><Heart size={14} />Sponsor</a
			>
			<a
				class="{linkClass} border-border-primary text-text-primary hover:bg-surface-elevated"
				href={ISSUES}
				target="_blank"
				rel="noopener noreferrer"><Bug size={14} />Report an issue</a
			>
		</div>
	</div>
</div>
