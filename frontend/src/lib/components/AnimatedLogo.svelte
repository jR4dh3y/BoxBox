<script lang="ts">
	/** The back plate emerges once; the front B's disk lights keep blinking. */
	let { busy = false }: { busy?: boolean } = $props();

	const layers = Array.from({ length: 80 }, (_, index) => ({
		x: -35 + index * 0.4375,
		y: -20 + index * 0.25
	}));

	let visible = $state(true);
	function syncVisibility() {
		visible = !document.hidden;
	}
</script>

<svelte:document onvisibilitychange={syncVisibility} />

<svg
	viewBox="0 0 570 410"
	fill="none"
	role="img"
	aria-label="BoxBox"
	class="h-full w-full"
	{@attach syncVisibility}
>
	<defs>
		<mask id="bbl-cutouts" maskUnits="userSpaceOnUse" x="0" y="0" width="480" height="256">
			<rect width="480" height="256" fill="white" />
			<rect x="80" y="68" width="40" height="28" rx="8" fill="black" />
			<rect x="80" y="160" width="64" height="28" rx="8" fill="black" />
			<circle cx="58" cy="82" r="6" fill="black" />
			<circle cx="58" cy="174" r="6" fill="black" />
		</mask>
		<g id="bbl-box" fill="currentColor">
			<g mask="url(#bbl-cutouts)">
				<path d="M40 40H124a20 20 0 0 1 20 20V104a20 20 0 0 1-20 20H40Z" />
				<path d="M40 136H148a20 20 0 0 1 20 20V196a20 20 0 0 1-20 20H40Z" />
			</g>
			<rect
				x="206"
				y="110"
				width="92"
				height="92"
				rx="26"
				fill="none"
				stroke="currentColor"
				stroke-width="28"
			/>
			<circle cx="252" cy="156" r="9" />
			<path
				d="M348 110 428 202M428 110 348 202"
				fill="none"
				stroke="currentColor"
				stroke-width="28"
				stroke-linecap="round"
			/>
		</g>
		<g id="bbl-face" transform="matrix(.95 -.38 0 1 110 170)">
			<use href="#bbl-box" />
		</g>
		<g id="bbl-plate">
			<g color="#205cad">
				{#each layers as layer, index (index)}
					<use href="#bbl-face" x={layer.x} y={layer.y} />
				{/each}
			</g>
			<use href="#bbl-face" />
		</g>
	</defs>
	<use
		href="#bbl-plate"
		x="-63"
		y="-36"
		color="#3278c9"
		class="animate-plate-emerge motion-reduce:animate-none"
	/>
	<use href="#bbl-plate" color="#4a9eff" />
	<!-- Status lights in the two holes of the B's server rack, on the front plate's face. -->
	<g transform="matrix(.95 -.38 0 1 110 170)" fill="#4ade80">
		<circle
			cx="58"
			cy="82"
			r="7"
			class="{busy ? 'animate-led-busy' : 'animate-led-a'} motion-reduce:animate-none"
			style:animation-play-state={visible ? 'running' : 'paused'}
		/>
		<circle
			cx="58"
			cy="174"
			r="7"
			class="{busy ? 'animate-led-busy' : 'animate-led-b'} motion-reduce:animate-none"
			style:animation-play-state={visible ? 'running' : 'paused'}
		/>
	</g>
</svg>
