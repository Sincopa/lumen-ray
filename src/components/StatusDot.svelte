<script lang="ts">
	interface Props {
		variant?: 'active' | 'expired';
		size?: number;
		className?: string;
	}

	let { variant = 'active', size = 8, className = '' }: Props = $props();
</script>

<span
	class="status-dot {className}"
	class:expired={variant === 'expired'}
	style="width: {size}px; height: {size}px;"
	aria-hidden="true"
></span>

<style>
	/* Figma: a 15% black inside rim and a 31px glow of the same green. */
	.status-dot {
		display: inline-block;
		border-radius: 50%;
		background: #3aee29;
		box-shadow:
			inset 0 0 0 1px #00000026,
			0 0 31px #3aee29;
		flex-shrink: 0;
	}

	.status-dot.expired {
		background: #ef3340;
		box-shadow:
			inset 0 0 0 1px #00000026,
			0 0 31px #ef3340;
	}
</style>
