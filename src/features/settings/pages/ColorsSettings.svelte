<script lang="ts">
	import SettingRow from '../../../components/SettingRow.svelte';
	import { t } from '../../../lib/i18n';
	import { shaderSettings } from '../../../lib/stores/shaderSettings.svelte';

	let exposureText = $state(shaderSettings.exposure.toFixed(2));
	let isEditingExposure = $state(false);

	$effect(() => {
		if (!isEditingExposure) {
			exposureText = shaderSettings.exposure.toFixed(2);
		}
	});

	function commitExposure() {
		isEditingExposure = false;
		const parsed = parseFloat(exposureText.replace(',', '.'));
		if (isNaN(parsed)) {
			exposureText = shaderSettings.exposure.toFixed(2);
			return;
		}
		const clamped = Math.min(2.5, Math.max(0.8, parsed));
		shaderSettings.setExposure(parseFloat(clamped.toFixed(2)));
		exposureText = clamped.toFixed(2);
	}
</script>

{#snippet colorRow(
	label: string,
	value: string,
	onInput: (hex: string) => void,
)}
	<SettingRow {label}>
		<div class="color-picker-control">
			<span class="color-hex">{value.toUpperCase()}</span>
			<label class="color-swatch" style:background-color={value}>
				<input
					type="color"
					aria-label={label}
					{value}
					oninput={(e) => onInput((e.target as HTMLInputElement).value)}
				/>
			</label>
		</div>
	</SettingRow>
{/snippet}

<div class="settings-group">
	{@render colorRow(
		t('settings.shaderIdleColor'),
		shaderSettings.idleColorA,
		(hex) => shaderSettings.setIdleColorA(hex),
	)}
	{@render colorRow(
		t('settings.shaderIdleColorB'),
		shaderSettings.idleColorB,
		(hex) => shaderSettings.setIdleColorB(hex),
	)}
	{@render colorRow(t('settings.shaderColorA'), shaderSettings.colorA, (hex) =>
		shaderSettings.setColorA(hex),
	)}
	{@render colorRow(t('settings.shaderColorB'), shaderSettings.colorB, (hex) =>
		shaderSettings.setColorB(hex),
	)}
	<SettingRow label={t('settings.shaderExposure')}>
		<input
			type="text"
			inputmode="decimal"
			class="value-input"
			aria-label="{t('settings.shaderExposure')} (x)"
			bind:value={exposureText}
			onfocus={() => (isEditingExposure = true)}
			onblur={commitExposure}
			onkeydown={(e) => {
				if (e.key === 'Enter') {
					(e.target as HTMLInputElement).blur();
				}
			}}
		/>
	</SettingRow>
</div>
<button type="button" class="wide-btn" onclick={() => shaderSettings.reset()}>
	{t('settings.shaderReset')}
</button>
