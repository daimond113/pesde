<script lang="ts">
	import type { PageData } from './$types';

	export let data: PageData;
</script>

<svelte:head>
	<title>pesde</title>
</svelte:head>

<section
	class="flex flex-col items-center lg:items-start text-center lg:text-start not-prose gap-6 my-4"
>
	<h1 class="text-3xl font-extrabold text-balance">
		pesde - the feature-rich Roblox package manager
	</h1>
	<div class="text-xl font-medium text-balance">
		pesde is a package manager for Roblox that is designed to be feature-rich and easy to use.
	</div>
	<div>
		<a
			href="https://github.com/daimond113/pesde?tab=readme-ov-file#installation"
			class="text-standard-text no-underline rounded-md px-4 py-2 bg-paper-1 inline-block"
			>Install</a
		>
		<a
			href="https://github.com/daimond113/pesde?tab=readme-ov-file#preparing-to-publish"
			class="text-standard-text no-underline rounded-md px-4 py-2 bg-paper-1-alt inline-block"
			>Publish</a
		>
	</div>
</section>

<section>
	<h2>Recently published packages</h2>
	<div class="grid grid-cols-1 lg:grid-cols-2 gap-2 overflow-auto">
		{#each data.latest as latestPackage}
			<a
				href="/packages/{latestPackage.name}/{latestPackage.version}"
				class="flex flex-col justify-center p-4 bg-paper-1 rounded-md text-standard-text no-underline not-prose h-32 *:overflow-text hover:brightness-110 transition-[filter]"
			>
				<span class="text-sm"
					>Published at <time datetime={latestPackage.published_at.toISOString()}
						>{latestPackage.published_at.toLocaleString()}</time
					></span
				>
				<div class="text-lg font-medium">{latestPackage.name}@{latestPackage.version}</div>
				{#if latestPackage.description}
					<div>{latestPackage.description}</div>
				{/if}
			</a>
		{/each}
	</div>
</section>
