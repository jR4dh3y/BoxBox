import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { formatFileDate } from './format';

describe('formatFileDate', () => {
	test('says Today for the current day so a time is never ambiguous', () => {
		assert.match(formatFileDate(new Date()), /^Today \d{1,2}:\d{2}\s?(AM|PM)$/);
	});

	test('shows the year only for another year', () => {
		const lastYear = new Date();
		lastYear.setFullYear(lastYear.getFullYear() - 1);
		assert.match(formatFileDate(lastYear), /\d{4}$/);
	});

	test('treats an unreadable date as missing', () => {
		assert.equal(formatFileDate('not a date'), '-');
	});
});
