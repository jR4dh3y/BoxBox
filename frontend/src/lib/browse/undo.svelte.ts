import { rename } from '#lib/api/files.js';
import { restoreDesktopTrashItem } from '#lib/api/desktopTrash.js';
import { toastStore } from '#lib/stores/toast.svelte.js';

type UndoAction =
	{ kind: 'trash'; ids: string[] } | { kind: 'rename'; before: string; after: string };
let action = $state.raw<UndoAction | null>(null);
let busy = $state(false);

export const desktopUndo = {
	get available() {
		return action !== null && !busy;
	},
	get label() {
		return action?.kind === 'trash' ? 'Undo Trash' : 'Undo Rename';
	},
	rememberTrash(ids: string[]) {
		action = { kind: 'trash', ids };
	},
	rememberRename(before: string, after: string) {
		action = { kind: 'rename', before, after };
	},
	async run(refresh: () => void) {
		if (!action || busy) return;
		const pending = action;
		busy = true;
		try {
			if (pending.kind === 'rename') await rename(pending.after, pending.before);
			else {
				for (const id of [...pending.ids]) {
					await restoreDesktopTrashItem(id);
					pending.ids = pending.ids.filter((item) => item !== id);
				}
			}
			if (action === pending) action = null;
			toastStore.success('Operation undone');
		} catch (error) {
			toastStore.error(error instanceof Error ? error.message : 'Could not undo this operation');
		} finally {
			busy = false;
			refresh();
		}
	}
};
