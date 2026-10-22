<script lang="ts">
	import { Button, Modal } from '#lib/components/ui/index.js';
	import type {
		ConflictingTransfer,
		TransferConflictDecision,
		TransferConflictResolution,
		TransferPlan
	} from '#lib/browse/transfers.js';

	interface Props {
		open: boolean;
		plan: TransferPlan | null;
		onapply: (decisions: TransferConflictDecision[]) => void;
		oncancelbatch: () => void;
	}

	let { open, plan, onapply, oncancelbatch }: Props = $props();

	let decisionState = $state.raw<{
		plan: TransferPlan | null;
		values: Record<number, TransferConflictResolution>;
	}>({ plan: null, values: {} });
	const decisions = $derived(decisionState.plan === plan ? decisionState.values : {});

	const conflicts = $derived(
		plan?.entries.filter((entry): entry is ConflictingTransfer => entry.kind === 'conflict') ?? []
	);
	const transferCount = $derived(
		plan?.entries.filter((entry) => {
			if (entry.kind === 'ready') return true;
			return (decisions[entry.id] ?? 'skip') === 'keep-both';
		}).length ?? 0
	);

	function choose(entryId: number, resolution: TransferConflictResolution): void {
		decisionState = {
			plan,
			values: { ...decisions, [entryId]: resolution }
		};
	}

	function cancelBatch(): void {
		decisionState = { plan: null, values: {} };
		oncancelbatch();
	}

	function decisionFor(entry: ConflictingTransfer): TransferConflictResolution {
		return decisions[entry.id] ?? 'skip';
	}

	function apply(): void {
		const resolved = conflicts.map((entry) => ({
			entryId: entry.id,
			resolution: decisionFor(entry)
		}));
		onapply(resolved);
	}

	function choiceClass(selected: boolean): string {
		return selected
			? 'border border-accent bg-accent text-white hover:bg-accent-hover'
			: 'border border-border-primary bg-surface-secondary text-text-secondary hover:bg-surface-tertiary hover:text-text-primary';
	}
</script>

<Modal
	{open}
	title="Already in {plan?.destinationDirectory.split('/').at(-1) || '/'}"
	size="lg"
	onclose={cancelBatch}
>
	<ul
		class="m-0 max-h-[50vh] list-none overflow-y-auto rounded border border-border-secondary p-0"
		aria-label="Destination name conflicts"
	>
		{#each conflicts as entry (entry.id)}
			{@const keepBothName = entry.keepBothPath.slice(entry.keepBothPath.lastIndexOf('/') + 1)}
			<li
				class="flex items-center gap-3 border-b border-border-secondary px-3 py-1.5 text-[13px] last:border-b-0"
			>
				<span class="min-w-0 flex-1 truncate text-text-primary" title={entry.source.path}>
					{entry.source.name}
				</span>
				<div class="flex shrink-0 gap-1">
					<button
						type="button"
						class="rounded px-2.5 py-1 text-xs transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-focus {choiceClass(
							decisionFor(entry) === 'skip'
						)}"
						aria-pressed={decisionFor(entry) === 'skip'}
						aria-label="Skip {entry.source.name}"
						onclick={() => choose(entry.id, 'skip')}
					>
						Skip
					</button>
					<button
						type="button"
						class="rounded px-2.5 py-1 text-xs transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-focus {choiceClass(
							decisionFor(entry) === 'keep-both'
						)}"
						aria-pressed={decisionFor(entry) === 'keep-both'}
						aria-label="Keep both copies of {entry.source.name}"
						title="Saves the copy as {keepBothName}"
						onclick={() => choose(entry.id, 'keep-both')}
					>
						Keep both
					</button>
				</div>
			</li>
		{/each}
	</ul>

	{#snippet footer()}
		<Button variant="primary" disabled={!plan} onclick={apply}>
			{transferCount === 0
				? 'Skip all'
				: `Continue with ${transferCount} ${transferCount === 1 ? 'item' : 'items'}`}
		</Button>
	{/snippet}
</Modal>
