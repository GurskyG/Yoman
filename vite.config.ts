import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({
				fallback: 'index.html'
			})
		})
	],
	clearScreen: false,
	envPrefix: ['VITE_', 'TAURI_'],
	server: {
		port: 1420,
		strictPort: true,
		host: process.env.TAURI_DEV_HOST || false,
		watch: {
			ignored: ['**/src-tauri/**']
		}
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}', 'src/**/*.editor.spec.ts']
				}
			},
			{
				extends: './vite.config.ts',
				test: {
					name: 'editor',
					environment: 'happy-dom',
					include: ['src/**/*.editor.spec.ts']
				}
			}
		]
	}
});
