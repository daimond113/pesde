import MarkdownIt from 'markdown-it';
import Shiki from '@shikijs/markdown-it';
import { writable } from 'svelte/store';

// nasty hack to get around the fact that @shikijs/markdown-it is async
export const md = writable<MarkdownIt | undefined>(undefined);

const it = MarkdownIt({
	html: true
});

Promise.all([Shiki({ theme: 'vesper' })]).then((plugins) => {
	for (const plugin of plugins) {
		it.use(plugin);
	}

	md.set(it);
});
