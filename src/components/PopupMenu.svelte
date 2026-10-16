<script lang="ts">
	import type { Snippet } from 'svelte';
	import { pop } from '../lib/motion';

	interface Props {
		top?: string;
		right?: string;
		children?: Snippet;
	}

	let { top = '68px', right = '8px', children }: Props = $props();
</script>

<!-- Grows from its trigger (top right) and leaves faster than it arrives. -->
<div
	class="popup-menu"
	style="top: {top}; right: {right};"
	in:pop={{ start: 0.94, duration: 200 }}
	out:pop={{ start: 0.97, duration: 120 }}
>
	{@render children?.()}
</div>

<style>
	.popup-menu {
		position: absolute;
		width: 197px;
		background: #ffffff0a;
		outline: none;
		border-radius: 24px;
		padding: 10px 12px;
		-webkit-backdrop-filter: blur(55px);
		backdrop-filter: blur(55px);
		/* box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5); */
		display: flex;
		flex-direction: column;
		gap: 6px;
		z-index: 100;
		transform-origin: top right;
		box-sizing: border-box;
	}
</style>
