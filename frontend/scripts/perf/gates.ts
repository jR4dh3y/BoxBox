export type BrowserMetrics = {
	listingRequests: number;
	/** File rows in the DOM once the 10,000-entry folder is open; virtualization bounds it. */
	renderedRows: number;
	refreshRequests: number;
	scriptRequests: number;
	scriptBytes: number;
};

export function browserViolations(metrics: BrowserMetrics, limits: BrowserMetrics): string[] {
	return Object.entries(limits).flatMap(([key, limit]) => {
		const value = Object.entries(metrics).find(([name]) => name === key)?.[1];
		if (typeof value !== 'number' || !Number.isFinite(value) || value > limit) {
			return [`${key}: ${value} exceeds ${limit}`];
		}
		return [];
	});
}

function numericLimits(value: unknown): Map<string, number> {
	if (typeof value !== 'object' || value === null || Array.isArray(value)) {
		throw new Error('Budget must be an object of finite non-negative numbers');
	}
	return new Map(
		Object.entries(value).map(([key, limit]) => {
			if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 0) {
				throw new Error(`${key}: budget must be finite and non-negative`);
			}
			return [key, limit];
		})
	);
}

/**
 * Budgets may only tighten. A key may disappear only when `retired` names the key that now
 * measures the same work, and that successor is no looser than the limit it replaces.
 */
export function checkRatchets(
	base: unknown,
	head: unknown,
	retired: Readonly<Record<string, string>> = {}
): string[] {
	const oldLimits = numericLimits(base);
	const newLimits = numericLimits(head);
	return [...oldLimits].flatMap(([key, limit]) => {
		const successor = Object.entries(retired).find(([name]) => name === key)?.[1];
		const name = newLimits.has(key) || successor === undefined ? key : successor;
		const next = newLimits.get(name);
		return next === undefined || next > limit ? [`${key}: ${limit} → ${next ?? 'deleted'}`] : [];
	});
}

export function median(values: number[]): number {
	if (values.length === 0 || values.some((value) => !Number.isFinite(value))) {
		throw new Error('Median requires finite samples');
	}
	const sorted = [...values].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

type Benchmark = { bytes: number[]; allocations: number[] };
function parseBenchmarks(text: string): Map<string, Benchmark> {
	const samples = new Map<string, Benchmark>();
	for (const line of text.split('\n')) {
		const match =
			/^(Benchmark\S+)\s+\d+\s+[\d.]+ ns\/op\s+([\d.]+) B\/op\s+([\d.]+) allocs\/op\s*$/.exec(line);
		if (!match) continue;
		const [, name, bytes, allocations] = match;
		const sample = samples.get(name) ?? { bytes: [], allocations: [] };
		sample.bytes.push(Number(bytes));
		sample.allocations.push(Number(allocations));
		samples.set(name, sample);
	}
	return samples;
}

// Go's per-iteration byte accounting can vary slightly with GC; ceilings in the
// service tests remain the absolute ratchet. Timing is reported by benchstat only.
export function benchmarkRegressions(
	baseText: string,
	headText: string,
	ceilings: Record<string, number> = {}
): string[] {
	const base = parseBenchmarks(baseText);
	const head = parseBenchmarks(headText);
	if (base.size === 0) throw new Error('No benchmarks in baseline');
	if (base.size !== head.size) throw new Error('Missing benchmarks in comparison');
	const violations: string[] = [];
	for (const [name, before] of base) {
		const after = head.get(name);
		if (!after) throw new Error(`Missing benchmark ${name}`);
		if (before.bytes.length < 10 || after.bytes.length < 10) {
			throw new Error(`${name}: at least 10 samples per revision required`);
		}
		for (const [unit, oldSamples, newSamples] of [
			['B/op', before.bytes, after.bytes],
			['allocs/op', before.allocations, after.allocations]
		] as const) {
			const previous = median(oldSamples);
			const current = median(newSamples);
			const key = `${name.replace(/^Benchmark/, '').replace(/-\d+$/, '')}.${unit === 'B/op' ? 'bytes' : 'allocations'}`;
			const ceiling = ceilings[key];
			if (Object.keys(ceilings).length && ceiling === undefined) {
				throw new Error(`Missing absolute ceiling ${key}`);
			}
			if (ceiling !== undefined && current > ceiling) {
				violations.push(`${name} ${unit}: ${current} exceeds ceiling ${ceiling}`);
			}
			if (current > previous * 1.01) {
				violations.push(`${name} ${unit}: ${previous} → ${current} (>1% growth)`);
			}
		}
	}
	return violations;
}
