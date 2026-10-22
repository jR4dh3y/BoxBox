import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const initialModules = new Set<string>();

export default defineConfig({
	plugins: [
		{
			name: 'startup-module-tags',
			buildStart() {
				initialModules.clear();
			}
		},
		tailwindcss(),
		sveltekit({
			preprocess: vitePreprocess(),

			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'font-src': ['self', 'data:'],
					'img-src': ['self', 'data:', 'blob:'],
					'media-src': ['self', 'blob:'],
					'frame-src': ['self', 'blob:'],
					'worker-src': ['self', 'blob:'],
					'connect-src': ['self', 'ws:', 'wss:'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'form-action': ['self']
				}
			},

			// Static adapter for single-container Docker deployment
			// Frontend is embedded in Go binary via go:embed
			adapter: adapter({
				pages: 'build',
				assets: 'build',
				fallback: 'index.html', // SPA fallback for client-side routing
				precompress: true, // Generate .gz and .br files
				strict: false // Don't fail on dynamic routes
			})
		})
	],
	build: {
		rolldownOptions: {
			output: {
				codeSplitting: {
					groups: [
						// Tag first, then group only dependencies used by the browse shell.
						{
							name(id) {
								initialModules.add(id);
								return null;
							},
							tags: ['$initial']
						},
						{
							name(id, ctx) {
								// Keep Kit's payload bootstrap separate from the client it starts.
								if (id.endsWith('/runtime/client/entry.js')) return null;
								if (id.endsWith('/runtime/client/client-entry.js')) return 'app';
								if (!initialModules.has(id) || !/node_modules[\\/]|[\\/]src[\\/]lib[\\/]/.test(id))
									return null;
								const visited = new Set<string>();
								const pending = [id];
								while (pending.length) {
									const next = pending.pop();
									if (!next || visited.has(next)) continue;
									visited.add(next);
									if (
										/src\/routes\/(?:\+layout|browse\/\+page)\.svelte$/.test(next) ||
										/(?:desktop\/BrowsePane|components\/Toolbar)\.svelte$/.test(next)
									)
										return 'app';
									pending.push(...(ctx.getModuleInfo(next)?.importers ?? []));
								}
								return null;
							}
						},
						{ name: 'browse', test: /(?:desktop\/BrowsePane|components\/Toolbar)\.svelte$/ }
					]
				}
			}
		}
	}
});
