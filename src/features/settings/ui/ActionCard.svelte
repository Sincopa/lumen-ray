<script lang="ts">
	interface Props {
		label: string;
		value: string;
		action: string;
		onclick: () => void;
		disabled: boolean;
		success?: boolean;
		error?: boolean;
		/** Figma "settings button": the light button for an action worth taking. */
		primary?: boolean;
		/** 0-1 while a download runs: a 15% white fill grows under the label. */
		progress?: number | null;
	}
	let {
		label,
		value,
		action,
		onclick,
		disabled,
		success = false,
		error = false,
		primary = false,
		progress = null,
	}: Props = $props();
</script>

<div class="settings-group action-card">
	<div class="info-row">
		<span class="info-label">{label}</span>
		<span class="info-value" class:error-text={error}>{value}</span>
	</div>
	<button
		type="button"
		class="wide-btn"
		class:success
		class:primary
		class:progressing={progress !== null}
		{disabled}
		{onclick}
	>
		{#if progress !== null}
			<span
				class="wide-btn-fill"
				style:width="{Math.max(4, progress * 100)}%"
				aria-hidden="true"
			></span>
		{/if}
		<span class="wide-btn-label">{action}</span>
	</button>
</div>
