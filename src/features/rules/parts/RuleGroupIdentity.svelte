<script lang="ts">
	import { t } from '../../../lib/i18n';
	import MaterialIcon from '../../../components/MaterialIcon.svelte';
	import type { RuleGroup } from '../ruleGroups';

	interface Props {
		group: RuleGroup;
		label: string;
		subscriptionName?: string;
		open: boolean;
		onToggleOpen: () => void;
	}
	let { group, label, subscriptionName, open, onToggleOpen }: Props = $props();
</script>

<div class="rule-left">
	<div class="app-icon-wrapper domain-icon">
		<span class="globe-text" aria-hidden="true">
			<MaterialIcon
				name={group.kind === 'addresses' ? 'pin_drop' : 'public'}
				size={20}
			/>
		</span>
	</div>
	<button
		class="rule-text"
		aria-expanded={open}
		aria-label={t(open ? 'rules.groupCollapse' : 'rules.groupExpand', {
			name: label,
		})}
		onclick={onToggleOpen}
	>
		<span class="rule-name-line">
			<span class="rule-name">{label}</span>
			<MaterialIcon name={open ? 'expand_less' : 'expand_more'} size={18} />
		</span>
		{#if subscriptionName}
			<span class="rule-details">{subscriptionName}</span>
		{/if}
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
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	/* Same light disc as a single domain rule. */
	.domain-icon {
		background: #efefef;
		border-radius: 50%;
		color: #00000080;
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
		color: inherit;
	}
	.rule-name-line {
		display: flex;
		align-items: center;
		gap: 2px;
		min-width: 0;
	}
	.rule-name {
		font-size: 14px;
		line-height: 20px;
		font-weight: 400;
		letter-spacing: -0.28px;
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
</style>
