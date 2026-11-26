import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { FileCode, FileText, File, FileArchive, Folder } from 'lucide-svelte';
import { getFileIcon, getFileTint } from './fileTypes.js';

describe('file tints', () => {
	test("match the desktop app's tinted set", () => {
		assert.equal(getFileTint('photo.JPG', false), '#2dd4bf');
		assert.equal(getFileTint('clip.mkv', false), '#a78bfa');
		assert.equal(getFileTint('song.flac', false), '#fb923c');
		assert.equal(getFileTint('paper.pdf', false), '#f87171');
		assert.equal(getFileTint('main.rs', false), '#60a5fa');
		assert.equal(getFileTint('data.json', false), '#60a5fa');
		assert.equal(getFileTint('backup.tar', false), '#fbbf24');
		assert.equal(getFileTint('app.deb', false), '#fbbf24');
		assert.equal(getFileTint('budget.xlsx', false), '#4ade80');
		assert.equal(getFileTint('talk.pptx', false), '#fb7185');
	});

	test('folders, config, documents and unknown files stay plain', () => {
		assert.equal(getFileTint('photos', true), null);
		assert.equal(getFileTint('app.conf', false), null);
		assert.equal(getFileTint('letter.docx', false), null);
		assert.equal(getFileTint('notes.txt', false), null);
		assert.equal(getFileTint('README', false), null);
	});

	test('icons come from the same kinds', () => {
		assert.equal(getFileIcon('photos', true), Folder);
		assert.equal(getFileIcon('app.conf', false), FileCode);
		assert.equal(getFileIcon('paper.pdf', false), FileText);
		assert.equal(getFileIcon('backup.tar', false), FileArchive);
		assert.equal(getFileIcon('app.deb', false), File);
		assert.equal(getFileIcon('Makefile', false), FileCode);
		assert.equal(getFileIcon('README', false), File);
	});
});
