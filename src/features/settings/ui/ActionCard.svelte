<script lang="ts">
	interface Props {
		label: string;
		value: string;
		action: string;
		onclick: () => void;
		disabled: boolean;
		success?: boolean;
		error?: boolean;
	}
	let {
		label,
		value,
		action,
		onclick,
		disabled,
		success = false,
		error = false,
	}: Props = $props();
</script>

<div class="settings-group action-card">
	<div class="info-row">
		<span class="info-label">{label}</span>
		<span class="info-value" class:error-text={error}>{value}</span>
	</div>
	<button type="button" class="wide-btn" class:success {disabled} {onclick}>
		{action}
	</button>
</div>
