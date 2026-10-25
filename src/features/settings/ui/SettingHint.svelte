<script lang="ts">
	interface Props {
		text: string | undefined;
		alert?: boolean;
	}
	let { text, alert = false }: Props = $props();
</script>

{#if text}
	<p class="hint" role={alert ? 'alert' : undefined}>{text}</p>
{/if}
