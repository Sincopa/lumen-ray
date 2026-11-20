<script lang="ts">
	import type { Rule, RuleMatcher } from '../../../lib/services/catalog';
	import { t } from '../../../lib/i18n';
	import { formatRuleDisplayName } from '../ruleDisplay';
	import linkIcon from '../../../assets/icons/rule-link.svg';
	import appIcon from '../../../assets/icons/rule-app.svg';
	import geositeIcon from '../../../assets/icons/rule-geosite.svg';
	import geoipIcon from '../../../assets/icons/rule-geoip.svg';
	import tunIcon from '../../../assets/icons/mode-tun.svg';

	interface Props {
		rule: Rule;
		matcher: RuleMatcher;
		serverName: string;
		ruleIcon: string | null;
		/** A rule inside an opened group: text only, no icon. */
		nested?: boolean;
		onEdit: () => void;
	}
	let {
		rule,
		matcher,
		serverName,
		ruleIcon,
		nested = false,
		onEdit,
	}: Props = $props();

	const isGeo = $derived(matcher === 'geosite' || matcher === 'geoip');
	// Figma Component 68: the rule's kind as a dark glyph on a white disc.
	const kindIcon = $derived(
		matcher === 'geosite'
			? geositeIcon
			: matcher === 'geoip'
				? geoipIcon
				: matcher === 'final'
					? tunIcon
					: matcher === 'application'
						? appIcon
						: linkIcon,
	);
</script>

<div class="rule-left">
	{#if !nested}
		<div class="app-icon-wrapper" class:domain-icon={!ruleIcon}>
			{#if ruleIcon}
				<img class="app-icon" src={ruleIcon} alt="" />
			{:else}
				<img class="kind-icon" src={kindIcon} alt="" width="24" height="24" />
			{/if}
		</div>
	{/if}
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
	/* Generic rules: a white disc with the kind's glyph in 75% black. */
	.domain-icon {
		background: #fff;
		border-radius: 50%;
	}
	.app-icon {
		width: 32px;
		height: 32px;
		object-fit: cover;
		border-radius: 10px;
	}
	.kind-icon {
		display: block;
		filter: brightness(0);
		opacity: 0.75;
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
