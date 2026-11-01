import assert from 'node:assert/strict';
import { test } from 'node:test';
import { WALLPAPER_FILE_ACCEPT, WALLPAPER_MAX_BYTES, wallpaperFileProblem } from './wallpaperFile';

const file = (name: string, type: string, size = 1024) => ({ name, type, size });

test('wallpaper files match the types the server stores', () => {
	for (const picked of [
		file('photo.jpg', 'image/jpeg'),
		file('My Photo (1).PNG', 'image/png'),
		file('art.svg', 'image/svg+xml'),
		file('untyped.webp', ''),
		file('scan.avif', '')
	])
		assert.equal(wallpaperFileProblem(picked), null, picked.name);
	for (const picked of [
		file('photo.bmp', 'image/bmp'),
		file('photo.heic', 'image/heic'),
		file('notes.txt', 'text/plain'),
		file('renamed.png', 'application/pdf'),
		file('png', '')
	])
		assert.match(wallpaperFileProblem(picked) ?? '', /Choose a JPEG/, picked.name);
	assert.match(wallpaperFileProblem(file('empty.png', 'image/png', 0)) ?? '', /empty/);
	assert.match(
		wallpaperFileProblem(file('huge.png', 'image/png', WALLPAPER_MAX_BYTES + 1)) ?? '',
		/smaller than/
	);
	assert.equal(wallpaperFileProblem(file('limit.png', 'image/png', WALLPAPER_MAX_BYTES)), null);
	assert.ok(
		WALLPAPER_FILE_ACCEPT.includes('image/webp') && WALLPAPER_FILE_ACCEPT.includes('.jpeg')
	);
});
