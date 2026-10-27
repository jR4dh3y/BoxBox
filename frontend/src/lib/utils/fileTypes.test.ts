import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { FILE_EXTENSIONS, getPreviewType, type PreviewType } from './fileTypes';

const previews: Record<keyof typeof FILE_EXTENSIONS, PreviewType> = {
	video: 'video',
	audio: 'audio',
	image: 'image',
	pdf: 'pdf',
	code: 'code',
	text: 'text',
	archive: 'unsupported',
	spreadsheet: 'office',
	document: 'office',
	presentation: 'office',
	notebook: 'notebook',
	web: 'code',
	style: 'code',
	data: 'code'
};

describe('getPreviewType', () => {
	test('routes every known extension, including uppercase names, to its preview', () => {
		for (const [category, extensions] of Object.entries(FILE_EXTENSIONS)) {
			const expected = Object.entries(previews).find(([key]) => key === category)?.[1];
			assert.ok(expected);
			for (const extension of extensions) {
				assert.equal(getPreviewType(`example.${extension}`), expected, extension);
				assert.equal(getPreviewType(`example.${extension.toUpperCase()}`), expected, extension);
			}
		}
	});
	test('keeps special filenames and unknown files on their existing routes', () => {
		for (const name of ['Dockerfile', 'Makefile', '.bashrc', '.env']) {
			assert.equal(getPreviewType(name), 'code');
		}
		for (const name of ['README', 'example.unknown', 'example.zip']) {
			assert.equal(getPreviewType(name), 'unsupported');
		}
	});
});
