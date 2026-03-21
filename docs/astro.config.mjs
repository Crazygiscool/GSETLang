// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import vercel from '@astrojs/vercel';
import expressiveCode from 'astro-expressive-code';

export default defineConfig({
	output: 'static',
	adapter: vercel(),
	integrations: [
		expressiveCode({
			themes: ['dracula', 'github-dark-dimmed', 'one-dark-pro', 'nord'],
			themeCssSelector: (theme) => `[data-theme='${theme.name}']`,
			styleOverrides: {
				borderRadius: '0.5rem',
				frames: {
					shadowColor: '#00000066',
				},
			},
		}),
		starlight({
			title: 'GSET - Generic Syntax Extension Tool',
			description: 'Write in any language syntax, compile to any language',
			logo: {
				src: './src/assets/gset-logo.svg',
			},
			defaultLocale: 'root',
			locales: {
				root: { label: 'English', lang: 'en' },
			},
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/Crazygiscool/GSETLang' },
				{ icon: 'discord', label: 'Discord', href: 'https://discord.gg/gset' },
				{ icon: 'twitter', label: 'Twitter', href: 'https://twitter.com/gsetlang' },
			],
			editLink: {
				baseUrl: 'https://github.com/Crazygiscool/GSETLang/edit/main/',
			},
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Introduction', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Quick Start', slug: 'getting-started/quick-start' },
					],
				},
				{
					label: 'Core Concepts',
					items: [
						{ label: 'How It Works', slug: 'core-concepts/how-it-works' },
						{ label: 'Keyword Mapping', slug: 'core-concepts/keyword-mapping' },
						{ label: 'Configuration', slug: 'core-concepts/configuration' },
					],
				},
				{
					label: 'Language Support',
					autogenerate: { directory: 'languages' },
				},
				{
					label: 'Examples',
					items: [
						{ label: 'Basic Usage', slug: 'examples/basic-usage' },
						{ label: 'Custom Keywords', slug: 'examples/custom-keywords' },
						{ label: 'Multiple Languages', slug: 'examples/multiple-languages' },
					],
				},
				{
					label: 'Reference',
					autogenerate: { directory: 'reference' },
				},
			],
			customCss: ['./src/styles/custom.css', './src/styles/animations.css'],
			components: {
				Hero: './src/components/starlight/Hero.astro',
			},
		}),
	],
});