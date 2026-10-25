<script lang="ts">
	import { pop } from '../../../lib/motion';

	export interface ChoiceOption {
		value: string;
		label: string;
	}

	interface Props {
		options: ChoiceOption[];
		value: string;
		onSelect: (value: string) => void;
		disabled?: boolean;
	}
	let { options, value, onSelect, disabled = false }: Props = $props();
</script>

<div class="settings-group choice-group" role="radiogroup">
	{#each options as option (option.value)}
		<button
			type="button"
			role="radio"
			class="choice-row"
			aria-checked={value === option.value}
			{disabled}
			onclick={() => value !== option.value && onSelect(option.value)}
		>
			<span class="choice-mark" aria-hidden="true">
				{#if value === option.value}
					<span
						class="choice-dot"
						in:pop={{ start: 0.4, duration: 220 }}
					></span>
				{/if}
			</span>
			<span class="choice-label">{option.label}</span>
		</button>
	{/each}
</div>
