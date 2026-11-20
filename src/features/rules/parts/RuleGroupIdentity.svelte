<script lang="ts">
	import { t } from '../../../lib/i18n';
	import type { RuleGroup } from '../ruleGroups';
	import linkIcon from '../../../assets/icons/rule-link.svg';
	import geoipIcon from '../../../assets/icons/rule-geoip.svg';
	import chevronIcon from '../../../assets/icons/chevron-down.svg';

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
		<img
			class="kind-icon"
			src={group.kind === 'addresses' ? geoipIcon : linkIcon}
			alt=""
			width="24"
			height="24"
		/>
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
			{#if subscriptionName}
				<span class="rule-details">{subscriptionName}</span>
			{/if}
		</span>
		<img
			class="group-chevron"
			class:open
			src={chevronIcon}
			alt=""
			width="24"
			height="24"
		/>
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
	/* Same white disc as a single domain rule. */
	.domain-icon {
		background: #fff;
		border-radius: 50%;
	}
	.kind-icon {
		display: block;
		filter: brightness(0);
		opacity: 0.75;
	}
	/* Figma: the arrow icon after the name, flipped up while the group is open. */
	.group-chevron {
		flex-shrink: 0;
		transition: transform 200ms var(--ease-out);
	}
	.group-chevron.open {
		transform: rotate(180deg);
	}
	/* Name and subscription on the left, the arrow 16px after them. */
	.rule-text {
		display: flex;
		align-items: center;
		gap: 16px;
		min-width: 0;
		min-height: 0;
		background: none;
		border: 0;
		padding: 0;
		text-align: left;
		line-height: 14px;
		color: inherit;
	}
	.rule-name-line {
		display: flex;
		flex-direction: column;
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
