import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:net';
import { results } from './common';

async function availablePort(): Promise<number> {
	const listener = createServer();
	await new Promise<void>((resolve, reject) => {
		listener.once('error', reject);
		listener.listen(0, '127.0.0.1', resolve);
	});
	const address = listener.address();
	if (address === null || typeof address === 'string') throw new Error('No loopback port');
	await new Promise<void>((resolve, reject) =>
		listener.close((error) => (error ? reject(error) : resolve()))
	);
	return address.port;
}

export function createDirectoryFixture() {
	const root = mkdtempSync(join(tmpdir(), 'boxbox-browser-perf-'));
	const mount = join(root, 'mount');
	mkdirSync(mount, { mode: 0o700 });
	for (let index = 0; index < 10_000; index++) {
		writeFileSync(join(mount, `File-${String(index).padStart(5, '0')}.txt`), 'fixture\n');
	}
	return { root, mount, dispose: () => rmSync(root, { recursive: true, force: true }) };
}

export async function startServer(
	binary: string,
	root: string,
	mount: string,
	label: string,
	readOnly = true
) {
	const port = await availablePort();
	const config = join(root, `${label}.yaml`);
	writeFileSync(
		config,
		[
			`port: ${port}`,
			'host: "127.0.0.1"',
			'rate_limit_rps: 1000',
			`data_dir: ${JSON.stringify(join(root, `${label}-data`))}`,
			'mount_points:',
			'  - name: "perf"',
			`    path: ${JSON.stringify(mount)}`,
			`    read_only: ${readOnly}`
		].join('\n'),
		{ mode: 0o600 }
	);
	const env: Record<string, string> = {};
	for (const [key, value] of Object.entries(process.env)) {
		if (
			value !== undefined &&
			!key.startsWith('BOXBOX_') &&
			!key.startsWith('FM_') &&
			key !== 'CONFIG_PATH'
		)
			env[key] = value;
	}
	env.GOMAXPROCS = '2';
	env.BOXBOX_JWT_SECRET = randomBytes(32).toString('hex');
	// Ephemeral credentials live only in the child environment, never the config.
	const password = randomBytes(32).toString('hex');
	env.BOXBOX_USERS_perf = await Bun.password.hash(password, { algorithm: 'bcrypt', cost: 10 });
	const log = join(results, `${label}-server.log`);
	writeFileSync(log, '');
	const child = Bun.spawn([binary, '--config', config], {
		cwd: root,
		env,
		stdout: 'ignore',
		stderr: Bun.file(log)
	});
	const url = `http://127.0.0.1:${port}`;
	async function stop() {
		child.kill('SIGTERM');
		const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
		try {
			await child.exited;
		} finally {
			clearTimeout(timer);
		}
	}
	try {
		for (let attempt = 0; attempt < 100; attempt++) {
			if (child.exitCode !== null)
				throw new Error(
					`${label}: server exited (${child.exitCode}); see perf-results/${label}-server.log`
				);
			try {
				if ((await fetch(`${url}/health`, { signal: AbortSignal.timeout(500) })).ok)
					return { url, stop, credentials: { username: 'perf', password } };
			} catch {
				/* Server has not bound its port yet. */
			}
			await Bun.sleep(100);
		}
		throw new Error(`${label}: server readiness timed out`);
	} catch (error) {
		await stop();
		throw error;
	}
}
