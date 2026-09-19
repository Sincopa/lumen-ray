<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		top?: string;
		right?: string;
		children?: Snippet;
	}

	let { top = '68px', right = '8px', children }: Props = $props();
</script>

<div class="popup-menu" style="top: {top}; right: {right};">
	{@render children?.()}
</div>

<style>
	.popup-menu {
		position: absolute;
		width: 197px;
		background: rgba(0, 0, 0, 0.2);
		/* border: 1px solid rgba(255, 255, 255, 0.08); */
		outline: none;
		border-radius: 24px;
		padding: 10px;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		/* box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5); */
		display: flex;
		flex-direction: column;
		gap: 4px;
		z-index: 100;
		transform-origin: top right;
		animation: menu-in 180ms var(--ease-spring);
		box-sizing: border-box;
	}

	@keyframes menu-in {
		from {
			opacity: 0;
			transform: translateY(-4px) scale(0.96);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
</style>
