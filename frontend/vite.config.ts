import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	css: {
		// Tailwind v4 plugins are auto-detected from package.json
	},
	build: {
		rolldownOptions: {
			output: {
				// Default chunking splits the first load into about 25 small files, and the browser's six
				// connections per host make them queue. Two groups cut that to a handful. Only code that loads
				// on the first visit joins a group (`$initial`), so lazy code such as the editors stays out.
				// Vendor code never imports app code, so the chunks cannot loop; `bun run perf:budget` checks.
				codeSplitting: {
					groups: [
						{ name: 'vendor', test: /node_modules/, tags: ['$initial'] },
						{ name: 'app', test: /[\\/]src[\\/]lib[\\/]/, tags: ['$initial'] }
					]
				}
			}
		}
	}
});
