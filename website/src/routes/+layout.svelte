<script lang="ts">
	import { goto } from '$app/navigation';
	import '../app.css';
	import '@fontsource-variable/hepta-slab';
	import Autocomplete from 'simple-svelte-autocomplete';
	import Menu from 'lucide-svelte/icons/menu';
	import { onMount } from 'svelte';

	type SearchItem = {
		name: string;
		version: string;
		description: string;
	};

	const fetchSearchData = async (query: string) => {
		const request = await fetch(
			`${import.meta.env.VITE_API_URL}/v0/search?query=${encodeURIComponent(query)}`
		);

		return (await request.json()) as SearchItem[];
	};

	let selectedSearchItem: SearchItem | null = null;

	$: {
		if (selectedSearchItem) {
			goto(`/packages/${selectedSearchItem.name}/${selectedSearchItem.version}`);
		}
	}

	let linksOpen = false;
	let linksRef: HTMLDivElement;

	onMount(() => {
		const handleClick = (event: MouseEvent) => {
			if (linksOpen && !linksRef.contains(event.target as Node)) {
				linksOpen = false;
			}
		};

		document.addEventListener('click', handleClick);

		return () => {
			document.removeEventListener('click', handleClick);
		};
	});
</script>

<div class="flex flex-col px-8 lg:px-16 py-4 gap-8 items-center lg:*:max-w-6xl">
	<header
		class="flex-0 flex flex-col lg:flex-row relative items-center gap-4 lg:gap-0 min-h-12 w-full"
	>
		<div class="flex items-center gap-8 z-10">
			<a href="/" class="inline-block lg:absolute top-0 left-0">
				<img src="/logo.svg" alt="pesde" class="h-12" />
			</a>
			<div
				class="relative lg:absolute lg:right-0 lg:top-1/2 lg:-translate-y-1/2 flex items-center"
				bind:this={linksRef}
			>
				<button
					type="button"
					title="Toggle links"
					class="hover:brightness-110 transition-[filter]"
					on:click={() => {
						linksOpen = !linksOpen;
					}}
				>
					<Menu class="size-8" />
				</button>
				<div
					class="absolute top-8 right-0 bg-paper-1-alt z-10 flex flex-col gap-4 p-4 rounded-md *:no-underline *:text-standard-text hover:*:brightness-110 *:max-w-60"
					class:hidden={!linksOpen}
				>
					<a href="https://github.com/daimond113/pesde" class="w-max">GitHub Repository</a>
					<a href="/policies">Policies</a>
				</div>
			</div>
		</div>
		<Autocomplete
			inputClassName="mx-auto rounded-full text-white placeholder:opacity-75 placeholder:text-white bg-paper-1 px-3 py-1 w-full h-8 hover:brightness-110 transition-[filter]"
			dropdownClassName="!bg-paper-1-alt !border-none rounded-md not-prose !p-2"
			placeholder="search"
			searchFunction={fetchSearchData}
			delay={350}
			localFiltering={false}
			labelFieldName="name"
			valueFieldName="name"
			bind:selectedItem={selectedSearchItem}
			hideArrow={true}
		>
			<div slot="item" let:item>
				<div
					class="flex flex-col justify-center w-full no-underline text-standard-text transition-[filter] h-16"
				>
					<div class="font-bold text-lg overflow-text">{item?.name}</div>
					{#if item?.description}
						<div class="overflow-text">
							{item.description}
						</div>
					{/if}
				</div>
			</div>
		</Autocomplete>
	</header>

	<div class="prose prose-pesde w-full flex-1 flex-shrink-0">
		<slot />
	</div>
</div>

<style>
	:global(.autocomplete) {
		margin-left: auto;
		margin-right: auto;
		max-width: 25rem !important;
	}

	:global(.autocomplete-list-item) {
		background: #4c3c2d !important;
		color: unset !important;
	}

	:global(.autocomplete-list-item):hover {
		filter: brightness(1.1);
	}

	:global(.autocomplete-list-item-no-results) {
		color: unset !important;
	}
</style>
