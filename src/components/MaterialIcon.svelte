<script lang="ts">
	interface Props {
		name: string;
		size?: number;
		fill?: boolean;
		class?: string;
	}

	let {
		name,
		size = 20,
		fill = false,
		class: className = '',
	}: Props = $props();
</script>

<span
	class="material-symbols-rounded {className}"
	style:font-size="{size}px"
	style:width="{size}px"
	style:height="{size}px"
	style:font-variation-settings="'FILL' {fill ? 1 : 0}, 'wght' 400, 'GRAD' 0,
	'opsz' {Math.min(48, Math.max(20, Math.round(size)))}"
	aria-hidden="true"
>
	{name}
</span>

<style>
	.material-symbols-rounded {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		line-height: 1;
		user-select: none;
		vertical-align: middle;
		flex-shrink: 0;
		overflow: hidden;
	}
</style>
