<script lang="ts">
	import { t, i18n } from '../../../lib/i18n';
	import type { Subscription } from '../../../lib/services/catalog';
	import { refreshStatus } from '../../../lib/services/maintenance';
	import { formatSubscriptionUpdatedAgo } from '../../../lib/utils/time';

	interface Props {
		sub: Subscription;
	}
	let { sub }: Props = $props();

	const subscriptionRefreshStatus = refreshStatus;

	function formatUpdatedAgo(subscription: Subscription): string {
		const minutes =
			subscription.updatedMinutesAgo ??
			Math.max(
				1,
				Math.floor(
					(Date.now() - (subscription.updatedAt || Date.now() - 60_000)) /
						60_000,
				),
			);
		if (minutes < 60)
			return t('subscriptions.updatedMinutesAgo', { count: minutes });
		return formatSubscriptionUpdatedAgo(
			subscription.updatedAt,
			subscription.updatedMinutesAgo,
			i18n.currentLanguage,
		);
	}
</script>

<div class="card-details">
	<div class="metrics-row">
		<div class="metric-item">
			<span class="metric-label">
				{t('subscriptions.daysLabel')}
			</span>
			<span class="metric-val">
				{sub.daysRemaining ?? (sub.traffic === '∞' ? '∞' : '—')}
			</span>
		</div>
		<div class="metric-item">
			<span class="metric-label">
				{t('subscriptions.trafficLabel')}
			</span>
			<span class="metric-val">{sub.traffic ?? '—'}</span>
		</div>
	</div>
	<div class="updated-time">{formatUpdatedAgo(sub)}</div>
	{#if $subscriptionRefreshStatus[sub.id]?.state === 'error'}
		<div class="refresh-error" role="status">
			{t('subscriptions.refreshFailed')}: {t(
				'errors.' + ($subscriptionRefreshStatus[sub.id].error ?? 'unknown'),
			)}
		</div>
	{/if}
</div>

<style>
	.card-details {
		display: flex;
		width: 100%;
		justify-content: space-between;
		align-items: flex-end;
		gap: 12px;
	}
	.metrics-row {
		display: flex;
		gap: 24px;
	}
	.metric-item:first-child {
		min-width: 43px;
	}
	.metric-item {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.metric-label {
		font-size: 12px;
		line-height: 12px;
		font-weight: 500;
		color: #ffffff40;
	}
	.metric-val {
		font-size: 12px;
		line-height: 12px;
		font-weight: 500;
		letter-spacing: -0.24px;
	}
	.updated-time {
		font-size: 12px;
		line-height: 14px;
		color: #ffffff40;
		max-width: 50%;
		text-align: right;
	}
	.refresh-error {
		font-size: 11px;
		color: #ffb4aa;
		text-align: right;
	}
</style>
