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
			themes: ['github-dark-dimmed'],
			themeCssSelector: () => '[data-theme]',
			styleOverrides: {
				borderRadius: '0.5rem',
				frames: {
					shadowColor: '#00000066',
				},
			},
		}),
		starlight({
			title: 'GSET - Generic Syntax Extension Tool',
			description: 'One source. Every runtime. Write once in GSET, transpile to Python, JavaScript, Go, Java, or Ruby.',
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
					label: 'Language Guide',
					items: [
						{ label: 'Overview', slug: 'language/overview' },
						{ label: 'Variables', slug: 'language/variables' },
						{ label: 'Operators', slug: 'language/operators' },
						{ label: 'Arrays & Maps', slug: 'language/arrays-maps' },
						{ label: 'Control Flow', slug: 'language/control-flow' },
						{ label: 'Loops', slug: 'language/loops' },
						{ label: 'Functions', slug: 'language/functions' },
						{ label: 'Classes', slug: 'language/classes' },
						{ label: 'Error Handling', slug: 'language/error-handling' },
						{ label: 'Modules', slug: 'language/modules' },
					],
				},
				{
					label: 'Targets',
					items: [
						{ label: 'Overview', slug: 'targets/overview' },
						{ label: 'Python', slug: 'targets/python' },
						{ label: 'JavaScript', slug: 'targets/javascript' },
						{ label: 'Go', slug: 'targets/go' },
						{ label: 'Java', slug: 'targets/java' },
						{ label: 'Ruby', slug: 'targets/ruby' },
						{ label: 'Planned', slug: 'targets/planned' },
					],
				},
				{
					label: 'Core Concepts',
					items: [
						{ label: 'How It Works', slug: 'core-concepts/how-it-works' },
						{ label: 'Configuration', slug: 'core-concepts/configuration' },
						{ label: 'Portability', slug: 'core-concepts/portability' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'CLI', slug: 'reference/cli' },
						{ label: 'Errors', slug: 'reference/errors' },
						{ label: 'Security', slug: 'security/security' },
					],
				},
				{
					label: 'Concepts',
					items: [
						{ label: 'Limitations', slug: 'concepts/limitations' },
						{ label: 'GSET vs Other Tools', slug: 'concepts/gset-vs-x' },
						{ label: 'Roadmap', slug: 'concepts/roadmap' },
					],
				},
				{
					label: 'Development',
					items: [
						{ label: 'Building', slug: 'development/building' },
						{ label: 'Architecture', slug: 'development/architecture' },
					],
				},
			],
			customCss: ['./src/styles/custom.css', './src/styles/animations.css'],
			components: {
				Hero: './src/components/starlight/Hero.astro',
				ThemeSelect: './src/components/starlight/ThemePicker.astro',
			},
		}),
	],
});