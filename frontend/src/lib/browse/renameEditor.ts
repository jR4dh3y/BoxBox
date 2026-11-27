/** What a list or grid needs to rename one of its items where the name is drawn. */
export interface RenameEditor {
	path: string;
	isDir: boolean;
	name: string;
	problem: string | null;
	oninput: (name: string) => void;
	oncommit: () => void;
	oncancel: () => void;
}
