import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { setSystemTime } from 'bun:test';
import { formatRelativeTime, formatFileDate } from './format';

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

describe('formatRelativeTime', () => {
	test('preserves past and future wording at rounded unit boundaries', () => {
		const now = new Date('2026-10-27T12:00:00Z');
		setSystemTime(now);
		try {
			const cases: [number, string][] = [
				[0, 'in a moment'],
				[-1, 'just now'],
				[59_499, 'in a moment'],
				[59_500, 'in 1 minute'],
				[-59_500, 'just now'],
				[-59_501, '1 minute ago'],
				[120_000, 'in 2 minutes'],
				[-120_000, '2 minutes ago'],
				[3_600_000, 'in 1 hour'],
				[-3_600_000, '1 hour ago'],
				[86_400_000, 'in 1 day'],
				[-86_400_000, '1 day ago'],
				[604_800_000, 'in 1 week'],
				[-604_800_000, '1 week ago'],
				[2_592_000_000, 'in 1 month'],
				[-2_592_000_000, '1 month ago'],
				[31_536_000_000, 'in 1 year'],
				[-31_536_000_000, '1 year ago']
			];
			for (const [offset, expected] of cases) {
				assert.equal(formatRelativeTime(now.getTime() + offset), expected);
			}
			assert.equal(formatRelativeTime(now.toISOString()), 'in a moment');
			assert.equal(formatRelativeTime('invalid'), 'Invalid date');
		} finally {
			setSystemTime();
		}
	});
});
