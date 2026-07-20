import type { Component } from 'svelte';
import type { PreviewType } from '$lib/utils/fileTypes';

type PreviewComponent = Component<Record<string, unknown>>;
type PreviewModule = { default: unknown };

const loaders: Partial<Record<PreviewType, () => Promise<PreviewModule>>> = {
	video: () => import('./VideoPreview.svelte'),
	audio: () => import('./AudioPreview.svelte'),
	image: () => import('./ImagePreview.svelte'),
	pdf: () => import('./PdfPreview.svelte'),
	office: () => import('./OfficePreview.svelte'),
	notebook: () => import('./NotebookPreview.svelte'),
	code: () => import('./CodePreview.svelte'),
	text: () => import('./CodePreview.svelte')
};

const componentPromises = new Map<PreviewType, Promise<PreviewComponent | null>>();

export function loadPreviewComponent(type: PreviewType): Promise<PreviewComponent | null> {
	const cached = componentPromises.get(type);
	if (cached) return cached;

	const loader = loaders[type];
	const promise = loader
		? loader().then((module) => module.default as PreviewComponent)
		: Promise.resolve(null);
	componentPromises.set(type, promise);
	return promise;
}
