<script context="module">
	import DOMPurify from 'isomorphic-dompurify';

	DOMPurify.addHook('afterSanitizeAttributes', function (node) {
		if (node.tagName === 'A') {
			node.setAttribute('target', '_blank');
			node.setAttribute('rel', 'noopener noreferrer');
		}
	});
</script>

<script lang="ts">
	import type { PageData } from './$types';
	import { md } from '$lib/markdown';
	import Codeblock from '$lib/Codeblock.svelte';
	import { goto } from '$app/navigation';
	import ChevronDown from 'lucide-svelte/icons/chevron-down';
	import Mail from 'lucide-svelte/icons/mail';
	import Globe from 'lucide-svelte/icons/globe';
	import Check from 'lucide-svelte/icons/check';
	import X from 'lucide-svelte/icons/x';

	export let data: PageData;

	$: markdown =
		data.readme &&
		DOMPurify.sanitize($md?.render(data.readme) ?? '', {
			FORBID_TAGS: ['script', 'style', 'audio', 'iframe', 'object', 'embed', 'canvas']
		});

	$: allDependencies = [
		[data.dependencies, 'Dependencies'],
		[data.peerDependencies, 'Peer Dependencies']
	] as const;
</script>

<svelte:head>
	<title>{data.scope}/{data.name}@{data.version}</title>
	<meta content="{data.scope}/{data.name}@{data.version} - pesde" property="og:title" />
	{#if data.description}
		<meta content={data.description} name="description" />
		<meta content={data.description} property="og:description" />
	{/if}
</svelte:head>

<div class="flex flex-col lg:flex-row">
	<div class="flex-shrink flex-grow pr-4">
		<div class="mb-4">
			<h1 class="mb-0">{data.scope}/{data.name}</h1>
			{#if data.description}
				<div class="lead mt-0 mb-0">{data.description}</div>
			{/if}
		</div>

		<main>{@html markdown}</main>
	</div>
	<div class="w-full lg:w-72 flex-none">
		<hr class="lg:hidden" />
		<div class="flex flex-col gap-4 lg:sticky top-4">
			<section>
				<label for="version-select" class="section-title">Version</label>
				<div class="relative">
					<select
						class="w-full h-full px-4 py-2 rounded-full bg-paper-1 text-standard-text appearance-none hover:brightness-110 transition-[filter]"
						title="Version"
						id="version-select"
						on:change={(event) => {
							goto(`/packages/${data.scope}/${data.name}/${event.target?.value}`);
						}}
					>
						{#each data.versions as version}
							<option value={version} selected={version === data.version}>{version}</option>
						{/each}
					</select>
					<ChevronDown class="absolute right-4 top-1/4 pointer-events-none" />
				</div>
			</section>
			<section>
				<div class="section-title">Published at</div>
				<div class="flex items-center gap-2">
					<time datetime={data.publishedAt.toISOString()}>{data.publishedAt.toLocaleString()}</time>
				</div>
			</section>
			<section>
				<div class="section-title">Installation</div>
				<Codeblock code="pesde add {data.scope}/{data.name}@{data.version}" />
			</section>
			{#if data.license}
				<section>
					<div class="section-title">License</div>
					<div>{data.license}</div>
				</section>
			{/if}
			{#if data.repository}
				<section>
					<div class="section-title">Repository</div>
					<a
						href={data.repository}
						target="_blank"
						rel="noopener noreferrer"
						class="block overflow-text">{data.repository}</a
					>
				</section>
			{/if}
			{#if data.authors}
				<section>
					<div class="section-title">Authors</div>
					<ul class="not-prose">
						{#each data.authors as author}
							<li class="flex">
								<span class="overflow-text pr-2">
									{author.name}
								</span>
								<div class="ml-auto flex items-center gap-4">
									{#if author.email}
										<a href="mailto:{author.email}" title="Email {author.name}">
											<Mail class="size-6" />
										</a>
									{/if}
									{#if author.url}
										<a href={author.url} title="Website of {author.name}">
											<Globe class="size-6" />
										</a>
									{/if}
								</div>
							</li>
						{/each}
					</ul>
				</section>
			{/if}
			{#if data.realm}
				<section>
					<div class="section-title">Realm</div>
					<div>{data.realm}</div>
				</section>
			{/if}
			{#each allDependencies as [dependencies, title]}
				{#if dependencies && dependencies.length > 0}
					<section>
						<div class="section-title">{title}</div>
						<ul class="not-prose">
							{#each dependencies as dependency}
								<li>
									{#if 'name' in dependency}
										<a
											href="/packages/{dependency.name}/latest"
											class="block overflow-text"
											title="View {dependency.name}"
										>
											{dependency.name}@{dependency.version}
										</a>
									{:else}
										{@const url = /.+\/.+/.test(dependency.repo)
											? `https://github.com/${dependency.repo}`
											: dependency.repo}
										<a href={url} class="block overflow-text" title="View {dependency.repo}">
											{dependency.repo}#{dependency.rev}
										</a>
									{/if}
								</li>
							{/each}
						</ul>
					</section>
				{/if}
			{/each}
			<section>
				<div class="section-title">Exports</div>
				<ul class="not-prose">
					<li>
						<div class="flex items-center">
							Library:
							{#if data.exports.lib}
								<Check class="size-6 text-green-500 inline-block ml-auto" />
							{:else}
								<X class="size-6 text-red-500 inline-block ml-auto" />
							{/if}
						</div>
					</li>
					<li>
						<div class="flex items-center">
							Binary:
							{#if data.exports.bin}
								<Check class="size-6 text-green-500 inline-block ml-auto" />
							{:else}
								<X class="size-6 text-red-500 inline-block ml-auto" />
							{/if}
						</div>
					</li>
				</ul>
			</section>
		</div>
	</div>
</div>

<style>
	.section-title {
		@apply text-xl font-semibold;
	}
</style>
