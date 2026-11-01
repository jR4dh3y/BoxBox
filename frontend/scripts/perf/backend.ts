import { copyFileSync, appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { benchmarkRegressions } from './gates';
import ceilings from '../../../backend/internal/service/perf-budget.json';
import { baseArgument, repo, results, run, withBaseline } from './common';

const ref = baseArgument();
if (!ref) throw new Error('Pass --base <revision> for same-runner benchmark comparison');
const pattern = '^(BenchmarkListDirectory|BenchmarkSearchWalk)$';

// A base revision from before the single-trip listing pages through a folder. This adapter opens
// the whole folder that way, so the base measures the same user work as `listWholeFolder` on head.
const pagingAdapter = `package service

import (
	"context"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func listWholeFolder(ctx context.Context, svc FileService, path string) (int, error) {
	seen := 0
	for page := 1; ; page++ {
		list, err := svc.List(ctx, path, model.ListOptions{Page: page, PageSize: 50, SortBy: "name", SortDir: "asc", IncludeHidden: true})
		if err != nil {
			return 0, err
		}
		seen += len(list.Items)
		if seen >= list.TotalCount || len(list.Items) == 0 {
			return seen, nil
		}
	}
}
`;

await withBaseline(ref, async (base, sha) => {
	// The identical fixture exercises both implementations, including a master
	// revision from before these benchmarks existed.
	const service = (root: string, name: string) => join(root, 'backend/internal/service', name);
	for (const name of [
		'file_bench_test.go',
		'search_bench_test.go',
		'perf_budget_test.go',
		'perf-budget.json'
	]) {
		copyFileSync(service(repo, name), service(base, name));
	}
	const pages = readFileSync(service(base, 'file.go'), 'utf8').includes('opts model.ListOptions');
	if (pages) writeFileSync(service(base, 'list_folder_test.go'), pagingAdapter);
	else copyFileSync(service(repo, 'list_folder_test.go'), service(base, 'list_folder_test.go'));
	const binaries = [join(results, 'backend-base.test'), join(results, 'backend-head.test')];
	for (const [index, root] of [base, repo].entries()) {
		run(['go', 'test', '-c', '-o', binaries[index], './internal/service'], join(root, 'backend'));
	}
	const files = [join(results, 'backend-base.txt'), join(results, 'backend-head.txt')];
	for (const file of files) writeFileSync(file, '');
	for (let sample = 0; sample < 10; sample++) {
		// Alternate the first variant to distribute warm-up and thermal noise.
		for (const index of sample % 2 ? [1, 0] : [0, 1]) {
			appendFileSync(
				files[index],
				run([
					binaries[index],
					'-test.run=^$',
					`-test.bench=${pattern}`,
					'-test.benchmem',
					'-test.benchtime=200ms',
					'-test.count=1'
				])
			);
		}
	}
	const texts = await Promise.all(files.map((file) => Bun.file(file).text()));
	for (const text of texts) {
		for (const name of ['ListDirectory', 'SearchWalk']) {
			if (!new RegExp(`^Benchmark${name}-2\\s`, 'm').test(text))
				throw new Error(`Missing required benchmark ${name}`);
		}
	}
	const report = run([
		process.env.BENCHSTAT ?? 'benchstat',
		`base=${files[0]}`,
		`head=${files[1]}`
	]);
	writeFileSync(join(results, 'benchstat.txt'), `Baseline: ${sha}\n\n${report}`);
	console.log(report);
	const violations = benchmarkRegressions(texts[0], texts[1], ceilings);
	writeFileSync(
		join(results, 'backend-gate.json'),
		JSON.stringify(
			{
				base: sha,
				head: run(['git', 'rev-parse', 'HEAD']).trim(),
				samplesPerRevision: 10,
				gomaxprocs: 2,
				violations
			},
			null,
			2
		)
	);
	if (violations.length) throw new Error(violations.join('\n'));
});
