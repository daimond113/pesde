<script context="module">
	import { codeToHtml } from 'shiki';
</script>

<script lang="ts">
	export let lang = 'bash';
	export let code: string;
</script>

{#await codeToHtml(code, { theme: 'vesper', lang, transformers: [{ pre(node) {
				this.addClassToHast(node, 'not-prose overflow-x-auto px-4 py-2 rounded-md');
			} }] }) then highlightedCode}
	{@html highlightedCode}
{/await}
