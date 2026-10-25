<script lang="ts">
	import type { RuleMatcher } from '../../../lib/services/catalog';
	import { t } from '../../../lib/i18n';
	import { reveal } from '../../../lib/motion';
	import type { RuleTypeOption } from './ruleEditorModel';

	interface Props {
		types: RuleTypeOption[];
		picked: RuleMatcher | null;
		onConfirm: () => void;
	}
	let { types, picked = $bindable(), onConfirm }: Props = $props();
</script>

<!-- Choose what the rule matches. -->
<div class="type-list" role="radiogroup" aria-label={t('rules.kind')}>
	{#each types as type (type.id)}
		{@const selected = picked === type.id}
		<button
			type="button"
			role="radio"
			class="type-row"
			class:selected
			aria-checked={selected}
			onclick={() => (picked = type.id)}
			ondblclick={() => {
				picked = type.id;
				onConfirm();
			}}
		>
			<span class="type-head">
				<span class="type-label">
					{type.label.startsWith('rules.') ? t(type.label) : type.label}
				</span>
				<span class="type-radio" aria-hidden="true">
					{#if selected}<span class="type-dot"></span>{/if}
				</span>
			</span>
			{#if selected}
				<span class="type-desc" transition:reveal>{t(type.desc)}</span>
			{/if}
		</button>
	{/each}
</div>
