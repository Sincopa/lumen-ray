<script lang="ts">
	import type { RuleMatcher } from '../../../lib/services/catalog';
	import { t } from '../../../lib/i18n';
	import linkIcon from '../../../assets/icons/rule-link.svg';
	import appIcon from '../../../assets/icons/rule-app.svg';
	import geositeIcon from '../../../assets/icons/rule-geosite.svg';
	import geoipIcon from '../../../assets/icons/rule-geoip.svg';
	import tunIcon from '../../../assets/icons/mode-tun.svg';
	import type { RuleTypeOption } from './ruleEditorModel';

	interface Props {
		types: RuleTypeOption[];
		picked: RuleMatcher | null;
		onConfirm: () => void;
	}
	let { types, picked = $bindable(), onConfirm }: Props = $props();

	const icons: Record<RuleMatcher, string> = {
		domain: linkIcon,
		application: appIcon,
		geosite: geositeIcon,
		geoip: geoipIcon,
		final: tunIcon,
	};
</script>

<!-- Figma "Выберите категорию": a row per kind; a tap opens its form. -->
<div class="type-step">
	<h2 class="screen-title">
		<span class="title-lead">{t('rules.pickTitleLead')}</span>
		<span>{t('rules.pickTitle')}</span>
	</h2>
	<div class="type-list" role="list" aria-label={t('rules.kind')}>
		{#each types as type (type.id)}
			<button
				type="button"
				class="type-row"
				class:selected={picked === type.id}
				title={t(type.desc)}
				onclick={() => {
					picked = type.id;
					onConfirm();
				}}
			>
				<img src={icons[type.id]} alt="" width="32" height="32" />
				<span class="type-label">
					{type.label.startsWith('rules.') ? t(type.label) : type.label}
				</span>
			</button>
		{/each}
	</div>
</div>
