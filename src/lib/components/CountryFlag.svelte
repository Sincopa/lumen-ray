<script lang="ts">
	import fi from 'flag-icons/flags/4x3/fi.svg';
	import nl from 'flag-icons/flags/4x3/nl.svg';
	import de from 'flag-icons/flags/4x3/de.svg';
	import se from 'flag-icons/flags/4x3/se.svg';
	import us from 'flag-icons/flags/4x3/us.svg';
	import gb from 'flag-icons/flags/4x3/gb.svg';
	import jp from 'flag-icons/flags/4x3/jp.svg';
	import ru from 'flag-icons/flags/4x3/ru.svg';

	const flags: Record<string, string> = { fi, nl, de, se, us, gb, jp, ru };

	interface Props {
		countryCode?: string; // 'fi', 'nl', 'de', 'us', etc.
		size?: number;
	}

	let { countryCode = 'fi', size = 32 }: Props = $props();
</script>

<div class="flag-circle" style="width: {size}px; height: {size}px;">
	{#if flags[countryCode.toLowerCase()]}
		<img class="flag-icon" src={flags[countryCode.toLowerCase()]} alt="" />
	{:else}
		<span aria-hidden="true">—</span>
	{/if}
</div>

<style>
	.flag-circle {
		border-radius: 50%;
		overflow: hidden;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		background: rgba(255, 255, 255, 0.1);
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
	}

	.flag-icon {
		width: 100% !important;
		height: 100% !important;
		line-height: 1em;
		object-fit: cover;
		background-position: center !important;
		border-radius: 50%;
		display: block;
	}
</style>
