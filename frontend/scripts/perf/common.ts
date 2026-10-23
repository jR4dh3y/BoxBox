import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checkRatchets } from './gates';

export const repo = resolve(import.meta.dirname, '../../..');
export const results = join(repo, 'perf-results');
mkdirSync(results, { recursive: true });

export function run(args: string[], cwd = repo, log?: string): string {
	const result = Bun.spawnSync(args, {
		cwd,
		stdout: 'pipe',
		stderr: 'pipe',
		env: { ...process.env, GOMAXPROCS: '2' }
	});
	const output = result.stdout.toString();
	if (log) writeFileSync(log, output + result.stderr.toString());
	if (result.exitCode !== 0)
		throw new Error(`${args[0]} failed (${result.exitCode}): ${result.stderr.toString()}`);
	return output;
}

// Budgets whose measured work no longer exists, mapped to the budget that replaced them. Folder
// listing stopped paging: per-page, per-sort costs became one whole-folder cost. Delete an entry
// once no supported base revision still has its key.
const retiredBudgets: Readonly<Record<string, string>> = {
	'list/name': 'list',
	'list/type': 'list',
	'list/size': 'list',
	'list/modTime': 'list',
	'ListDirectory/name.bytes': 'ListDirectory.bytes',
	'ListDirectory/type.bytes': 'ListDirectory.bytes',
	'ListDirectory/size.bytes': 'ListDirectory.bytes',
	'ListDirectory/modTime.bytes': 'ListDirectory.bytes',
	'ListDirectory/name.allocations': 'ListDirectory.allocations',
	'ListDirectory/type.allocations': 'ListDirectory.allocations',
	'ListDirectory/size.allocations': 'ListDirectory.allocations',
	'ListDirectory/modTime.allocations': 'ListDirectory.allocations'
};

export async function withBaseline<T>(
	ref: string,
	action: (base: string, sha: string) => Promise<T>
): Promise<T> {
	const sha = run(['git', 'rev-parse', '--verify', '--end-of-options', `${ref}^{commit}`]).trim();
	const root = mkdtempSync(join(tmpdir(), 'boxbox-perf-base-'));
	const base = join(root, 'base');
	mkdirSync(base);
	try {
		run(['git', 'archive', '--format=tar', '--output', join(root, 'base.tar'), sha]);
		run(['tar', '-xf', join(root, 'base.tar'), '-C', base]);
		for (const path of [
			'frontend/perf-budget.json',
			'frontend/scripts/perf/budget.json',
			'backend/internal/service/perf-budget.json'
		]) {
			checkRatchets({}, await Bun.file(join(repo, path)).json());
			const oldFile = Bun.file(join(base, path));
			if (!(await oldFile.exists())) continue; // First introduction of a ratchet.
			const violations = checkRatchets(
				await oldFile.json(),
				await Bun.file(join(repo, path)).json(),
				retiredBudgets
			);
			if (violations.length)
				throw new Error(`${path}: budgets may only decrease: ${violations.join('; ')}`);
		}
		return await action(base, sha);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
}

export function baseArgument(): string | undefined {
	const index = process.argv.indexOf('--base');
	if (index < 0) return undefined;
	const ref = process.argv[index + 1];
	if (!ref || ref.startsWith('--')) throw new Error('--base requires a revision');
	return ref;
}
