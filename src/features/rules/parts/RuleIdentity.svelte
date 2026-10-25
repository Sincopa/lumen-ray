<script lang="ts">
	import type { Rule, RuleMatcher } from '../../../lib/services/catalog';
	import { t } from '../../../lib/i18n';
	import MaterialIcon from '../../../components/MaterialIcon.svelte';
	import { formatRuleDisplayName } from '../ruleDisplay';

	interface Props {
		rule: Rule;
		matcher: RuleMatcher;
		serverName: string;
		ruleIcon: string | null;
		onEdit: () => void;
	}
	let { rule, matcher, serverName, ruleIcon, onEdit }: Props = $props();

	const isGeo = $derived(matcher === 'geosite' || matcher === 'geoip');
</script>

<div class="rule-left">
	<div class="app-icon-wrapper" class:domain-icon={!ruleIcon}>
		{#if ruleIcon}
			<img class="app-icon" src={ruleIcon} alt="" />
		{:else if matcher === 'geosite'}
			<span class="globe-text" aria-hidden="true">
				<MaterialIcon name="travel_explore" size={24} />
			</span>
		{:else if matcher === 'geoip'}
			<span class="globe-text" aria-hidden="true">
				<MaterialIcon name="pin_drop" size={20} />
			</span>
		{:else if matcher === 'final'}
			<span class="globe-text" aria-hidden="true">
				<MaterialIcon name="alt_route" size={20} />
			</span>
		{:else}
			<span class="globe-text" aria-hidden="true">
				<MaterialIcon name="description" size={20} />
			</span>
		{/if}
	</div>
	<button
		class="rule-text"
		onclick={onEdit}
		aria-label={t('rules.edit', { name: rule.name })}
	>
		<span class="rule-name" title={formatRuleDisplayName(rule.name)}>
			{matcher === 'final'
				? t('rules.everythingElse') || 'Всё остальное'
				: formatRuleDisplayName(rule.name)}
		</span>
		<span class="rule-details" class:domain-details={matcher === 'domain'}>
			{matcher === 'final'
				? t('rules.finalDetails') || 'MATCH'
				: isGeo
					? matcher === 'geosite'
						? 'GeoSite'
						: 'GeoIP'
					: serverName}
		</span>
	</button>
</div>

<style>
	.rule-left {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		flex: 1;
	}
	.app-icon-wrapper {
		width: 32px;
		height: 32px;
		border-radius: 10px;
		overflow: hidden;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	/* Generic rules use a light disc with a grey glyph. */
	.domain-icon {
		background: #efefef;
		border-radius: 50%;
		color: #00000080;
	}
	.app-icon {
		width: 32px;
		height: 32px;
		object-fit: cover;
		border-radius: 10px;
	}
	.globe-text {
		font-size: 18px;
	}
	.rule-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		background: none;
		border: 0;
		padding: 0;
		text-align: left;
		justify-content: center;
		line-height: 14px;
	}
	.rule-name {
		font-size: 14px;
		line-height: 20px;
		font-weight: 400;
		letter-spacing: -0.28px;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.rule-details {
		font-size: 10px;
		line-height: 14px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		letter-spacing: 0.5px;
		color: #ffffff80;
	}
	.domain-details {
		display: none;
	}
</style>
