import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
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
				// Group initial dependencies to limit request queues; editors stay lazy.
				// Kit's bootstrap must keep its dynamic runtime import so it can set the payload first.
				codeSplitting: {
					groups: [
						{
							name: 'app',
							test: /node_modules[\\/](?!\.bun[\\/]|@sveltejs[\\/]kit[\\/])|[\\/]src[\\/]lib[\\/]/,
							tags: ['$initial']
						}
					]
				}
			}
		}
	}
});
