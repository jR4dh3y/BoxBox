<script lang="ts">
	import {
		isPlaceAction,
		placeMenuItems,
		type Place,
		type PlaceAction
	} from '#lib/browse/placeMenu.js';
	import type { DropQuestion } from '#lib/browse/fileDrop.js';
	import type { HistoryPlace } from '#lib/browse/location.svelte.js';
	import type { TransferSource } from '#lib/browse/transfers.js';
	import ContextMenu from './ui/ContextMenu.svelte';

	let {
		dropQuestion,
		placeMenu,
		historyMenu,
		aboveSidebar,
		canPaste,
		placeLabel,
		onTransfer,
		onPlaceAction,
		onTravel,
		onCloseDrop,
		onClosePlace,
		onCloseHistory
	}: {
		dropQuestion: DropQuestion | null;
		placeMenu: { x: number; y: number; place: Place } | null;
		historyMenu: { x: number; y: number; places: HistoryPlace[] } | null;
		aboveSidebar: boolean;
		canPaste: boolean;
		placeLabel: (path: string) => string;
		onTransfer: (action: 'move' | 'copy', items: TransferSource[], destination: string) => void;
		onPlaceAction: (place: Place, action: PlaceAction) => void;
		onTravel: (index: number) => void;
		onCloseDrop: () => void;
		onClosePlace: () => void;
		onCloseHistory: () => void;
	} = $props();
</script>

{#if dropQuestion}
	{@const question = dropQuestion}
	<ContextMenu
		items={[
			...(question.canMove ? [{ id: 'move', label: 'Move Here' }] : []),
			{ id: 'copy', label: 'Copy Here' },
			{ id: 'separator', label: '', separator: true },
			{ id: 'cancel', label: 'Cancel' }
		]}
		x={question.x}
		y={question.y}
		onSelect={(id) => {
			if (id === 'move' || id === 'copy') onTransfer(id, question.items, question.destination);
		}}
		onClose={onCloseDrop}
	/>
{/if}

{#if placeMenu}
	{@const menu = placeMenu}
	<ContextMenu
		items={placeMenuItems(menu.place, canPaste)}
		{aboveSidebar}
		x={menu.x}
		y={menu.y}
		onSelect={(id) => {
			if (isPlaceAction(id)) onPlaceAction(menu.place, id);
		}}
		onClose={onClosePlace}
	/>
{/if}

{#if historyMenu}
	{@const menu = historyMenu}
	<ContextMenu
		items={menu.places.map((place) => ({
			id: String(place.index),
			label: placeLabel(place.path)
		}))}
		x={menu.x}
		y={menu.y}
		onSelect={(id) => onTravel(Number(id))}
		onClose={onCloseHistory}
	/>
{/if}
