/**
 * The component gallery the native app is compared against, parsed from
 * routes/dev/gallery/gallery.json. The GPUI gallery reads the same file.
 */

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg' | 'icon';
export type ProgressVariant = 'default' | 'info' | 'success' | 'warning' | 'danger';
export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface MenuEntry {
	id: string;
	label: string;
	icon?: string;
	shortcut?: string;
	disabled?: boolean;
	separator?: boolean;
}

export type GalleryComponent =
	| {
			kind: 'button';
			variant: ButtonVariant;
			size: ButtonSize;
			label?: string;
			icon?: string;
			disabled: boolean;
	  }
	| { kind: 'input'; value: string; placeholder: string; disabled: boolean }
	| { kind: 'toggle'; checked: boolean; disabled: boolean }
	| { kind: 'select'; value: string; swatch?: string }
	| { kind: 'progress'; value: number; size: 'sm' | 'md'; variant: ProgressVariant }
	| { kind: 'context-menu'; items: MenuEntry[] }
	| { kind: 'modal'; title: string; value: string; cancel: string; confirm: string }
	| { kind: 'toasts'; toasts: { type: ToastType; message: string }[] };

export interface GalleryCell {
	id: string;
	x: number;
	y: number;
	w: number;
	h: number;
	component: GalleryComponent;
}

export interface Gallery {
	width: number;
	height: number;
	cells: GalleryCell[];
}

type Record_ = Record<string, unknown>;

function record(value: unknown, where: string): Record_ {
	if (typeof value !== 'object' || value === null || Array.isArray(value)) {
		throw new Error(`${where} must be an object`);
	}
	return Object.fromEntries(Object.entries(value));
}

function str(value: unknown, where: string): string {
	if (typeof value !== 'string') throw new Error(`${where} must be a string`);
	return value;
}

function num(value: unknown, where: string): number {
	if (typeof value !== 'number' || !Number.isFinite(value))
		throw new Error(`${where} must be a number`);
	return value;
}

function optionalStr(value: unknown, where: string): string | undefined {
	return value === undefined ? undefined : str(value, where);
}

function bool(value: unknown): boolean {
	return value === true;
}

function oneOf<T extends string>(value: unknown, options: readonly T[], where: string): T {
	const found = options.find((option) => option === value);
	if (found === undefined) throw new Error(`${where} must be one of ${options.join(', ')}`);
	return found;
}

function list(value: unknown, where: string): unknown[] {
	if (!Array.isArray(value)) throw new Error(`${where} must be a list`);
	return value;
}

function component(kind: string, props: Record_, where: string): GalleryComponent {
	switch (kind) {
		case 'button':
			return {
				kind,
				variant: oneOf(
					props.variant,
					['primary', 'secondary', 'ghost', 'danger'],
					`${where}.variant`
				),
				size: oneOf(props.size, ['sm', 'md', 'lg', 'icon'], `${where}.size`),
				label: optionalStr(props.label, `${where}.label`),
				icon: optionalStr(props.icon, `${where}.icon`),
				disabled: bool(props.disabled)
			};
		case 'input':
			return {
				kind,
				value: optionalStr(props.value, `${where}.value`) ?? '',
				placeholder: optionalStr(props.placeholder, `${where}.placeholder`) ?? '',
				disabled: bool(props.disabled)
			};
		case 'toggle':
			return { kind, checked: bool(props.checked), disabled: bool(props.disabled) };
		case 'select':
			return {
				kind,
				value: str(props.value, `${where}.value`),
				swatch: optionalStr(props.swatch, `${where}.swatch`)
			};
		case 'progress':
			return {
				kind,
				value: num(props.value, `${where}.value`),
				size: oneOf(props.size, ['sm', 'md'], `${where}.size`),
				variant: oneOf(
					props.variant,
					['default', 'info', 'success', 'warning', 'danger'],
					`${where}.variant`
				)
			};
		case 'context-menu':
			return {
				kind,
				items: list(props.items, `${where}.items`).map((entry, index) => {
					const item = record(entry, `${where}.items[${index}]`);
					return {
						id: str(item.id, `${where}.items[${index}].id`),
						label: optionalStr(item.label, `${where}.items[${index}].label`) ?? '',
						icon: optionalStr(item.icon, `${where}.items[${index}].icon`),
						shortcut: optionalStr(item.shortcut, `${where}.items[${index}].shortcut`),
						disabled: bool(item.disabled),
						separator: bool(item.separator)
					};
				})
			};
		case 'modal':
			return {
				kind,
				title: str(props.title, `${where}.title`),
				value: str(props.value, `${where}.value`),
				cancel: str(props.cancel, `${where}.cancel`),
				confirm: str(props.confirm, `${where}.confirm`)
			};
		case 'toasts':
			return {
				kind,
				toasts: list(props.toasts, `${where}.toasts`).map((entry, index) => {
					const toast = record(entry, `${where}.toasts[${index}]`);
					return {
						type: oneOf(
							toast.type,
							['success', 'error', 'info', 'warning'],
							`${where}.toasts[${index}].type`
						),
						message: str(toast.message, `${where}.toasts[${index}].message`)
					};
				})
			};
		default:
			throw new Error(`${where} has an unknown kind ${kind}`);
	}
}

export function parseGallery(value: unknown): Gallery {
	const root = record(value, 'gallery');
	return {
		width: num(root.width, 'gallery.width'),
		height: num(root.height, 'gallery.height'),
		cells: list(root.cells, 'gallery.cells').map((entry, index) => {
			const cell = record(entry, `cells[${index}]`);
			const id = str(cell.id, `cells[${index}].id`);
			return {
				id,
				x: num(cell.x, `${id}.x`),
				y: num(cell.y, `${id}.y`),
				w: num(cell.w, `${id}.w`),
				h: num(cell.h, `${id}.h`),
				component: component(str(cell.kind, `${id}.kind`), record(cell.props, `${id}.props`), id)
			};
		})
	};
}
