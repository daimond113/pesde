import defaultTheme from 'tailwindcss/defaultTheme';

/** @type {import('tailwindcss').Config} */
export default {
	content: ['./src/**/*.{html,js,svelte,ts}'],
	theme: {
		extend: {
			colors: {
				'standard-text': '#f8e4d5',
				'main-background': '#13100F',
				'paper-1': '#422911',
				'paper-1-alt': '#4C3C2D',
				links: '#ffa360'
			},
			fontFamily: {
				serif: ['Hepta Slab Variable', defaultTheme.fontFamily.serif]
			},
			typography: ({ theme }) => ({
				pesde: {
					css: {
						'--tw-prose-body': theme('colors.standard-text'),
						'--tw-prose-headings': theme('colors.standard-text'),
						'--tw-prose-lead': theme('colors.orange[100]'),
						'--tw-prose-links': theme('colors.links'),
						'--tw-prose-bold': theme('colors.orange[400]'),
						'--tw-prose-counters': theme('colors.orange[300]'),
						'--tw-prose-bullets': theme('colors.orange[300]'),
						'--tw-prose-hr': theme('colors.orange[100]'),
						'--tw-prose-quotes': theme('colors.orange[300]'),
						'--tw-prose-quote-borders': theme('colors.orange[500]'),
						'--tw-prose-captions': theme('colors.orange[300]'),
						'--tw-prose-th-borders': theme('colors.orange[300]'),
						'--tw-prose-td-borders': theme('colors.orange[300]'),
						'--tw-prose-code': theme('colors.orange[300]')
					}
				}
			})
		}
	},
	plugins: [require('@tailwindcss/typography')]
};
