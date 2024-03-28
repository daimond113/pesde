<script lang="ts">
	let className = '';

	export { className as class };
</script>

<div class={'px-6 py-4 bg-paper-1-alt rounded-md ' + className}>
	<div class="text-2xl font-bold">Note</div>
	<slot />
</div>
