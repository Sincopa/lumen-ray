<script lang="ts">
	import { pop } from '../../../lib/motion';
	import { t } from '../../../lib/i18n';
	import {
		shaderSettings,
		SHADER_PRESETS,
	} from '../../../lib/stores/shaderSettings.svelte';
</script>

<div class="settings-group choice-group" role="radiogroup">
	{#each SHADER_PRESETS as preset (preset.id)}
		{@const selected = shaderSettings.selectedPreset === preset.id}
		<button
			type="button"
			role="radio"
			class="choice-row"
			aria-checked={selected}
			onclick={() => shaderSettings.applyPreset(preset.id)}
		>
			<span class="choice-mark" aria-hidden="true">
				{#if selected}
					<span
						class="choice-dot"
						in:pop={{ start: 0.4, duration: 220 }}
					></span>
				{/if}
			</span>
			<span class="choice-label">{t(preset.nameKey)}</span>
			<span class="swatches-pair" aria-hidden="true">
				<span class="swatch-dot" style:background={preset.colorA}></span>
				<span
					class="swatch-dot swatch-overlap"
					style:background={preset.colorB}
				></span>
			</span>
		</button>
	{/each}
	{#if shaderSettings.selectedPreset === 'custom'}
		<div class="choice-row" role="radio" aria-checked="true">
			<span class="choice-mark" aria-hidden="true">
				<span class="choice-dot"></span>
			</span>
			<span class="choice-label">
				{t('settings.shaderPresetCustom')}
			</span>
			<span class="swatches-pair" aria-hidden="true">
				<span
					class="swatch-dot"
					style:background={shaderSettings.colorA}
				></span>
				<span
					class="swatch-dot swatch-overlap"
					style:background={shaderSettings.colorB}
				></span>
			</span>
		</div>
	{/if}
</div>
