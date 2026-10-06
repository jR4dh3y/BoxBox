// Lets `bun test` import `.svelte.ts` modules, which hold runes and need the Svelte compiler.
import { plugin } from 'bun';
import { readFileSync } from 'node:fs';
import { compileModule } from 'svelte/compiler';

const transpiler = new Bun.Transpiler({ loader: 'ts' });

plugin({
	name: 'svelte-modules',
	setup(build) {
		build.onLoad({ filter: /\.svelte\.(ts|js)$/ }, ({ path }) => {
			const source = path.endsWith('.ts')
				? transpiler.transformSync(readFileSync(path, 'utf8'))
				: readFileSync(path, 'utf8');
			const { js } = compileModule(source, { filename: path, generate: 'client' });
			return { contents: js.code, loader: 'js' };
		});
	}
});
