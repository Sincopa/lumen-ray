<script lang="ts">
	import { onMount } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, scale, slide } from 'svelte/transition';
	import { t } from '../../i18n';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import StatusDot from '../../components/StatusDot.svelte';

	interface Props {
		appliedPresetIds: Set<string>;
		hasActiveProxy: boolean;
		onApply: (presetId: string) => Promise<void>;
		onRemove: (presetId: string) => Promise<void>;
		onClose: () => void;
	}

	let {
		appliedPresetIds,
		hasActiveProxy,
		onApply,
		onRemove,
		onClose,
	}: Props = $props();

	let busyId = $state<string | null>(null);
	let expandedPresetId = $state<string | null>(null);
	let closeButton = $state<HTMLButtonElement | null>(null);

	onMount(() => {
		const previous = document.activeElement as HTMLElement | null;
		closeButton?.focus();
		return () => previous?.focus();
	});

	function toggleDetails(presetId: string) {
		expandedPresetId = expandedPresetId === presetId ? null : presetId;
	}

	interface PresetHighlight {
		icon: string;
		labelKey: string;
		defaultLabel: string;
		target: 'vpn' | 'direct';
	}

	interface PresetDef {
		id: string;
		icon: string;
		titleKey: string;
		defaultTitle: string;
		descKey: string;
		defaultDesc: string;
		requiresProxy: boolean;
		highlights: PresetHighlight[];
		tags: Array<{ type: 'geoip' | 'geosite'; tag: string; target: 'vpn' | 'direct' }>;
	}

	const PRESETS: PresetDef[] = [
		{
			id: 'russia-optimized',
			icon: 'auto_awesome',
			titleKey: 'presets.russiaOptimizedTitle',
			defaultTitle: 'Россия — Оптимальный',
			descKey: 'presets.russiaOptimizedDesc',
			defaultDesc:
				'Обход блокировок и зарубежных ограничений с сохранением прямого доступа к локальным сетям и российским сервисам.',
			requiresProxy: true,
			highlights: [
				{
					icon: 'shield',
					labelKey: 'presets.hlAntifilter',
					defaultLabel: 'Блокировки и фильтры',
					target: 'vpn',
				},
				{
					icon: 'public',
					labelKey: 'presets.hlRuDirect',
					defaultLabel: 'Российские сервисы',
					target: 'direct',
				},
				{
					icon: 'home',
					labelKey: 'presets.hlPrivateDirect',
					defaultLabel: 'Локальные сети (LAN)',
					target: 'direct',
				},
			],
			tags: [
				{ type: 'geoip', tag: 'private', target: 'direct' },
				{ type: 'geosite', tag: 'antifilter', target: 'vpn' },
				{ type: 'geosite', tag: 'antifilter-community', target: 'vpn' },
				{ type: 'geoip', tag: 'antifilter', target: 'vpn' },
				{ type: 'geoip', tag: 'antifilter-community', target: 'vpn' },
				{ type: 'geosite', tag: 'category-ru', target: 'direct' },
				{ type: 'geoip', tag: 'ru', target: 'direct' },
			],
		},
		{
			id: 'bypass-blocking',
			icon: 'vpn_lock',
			titleKey: 'presets.bypassBlockingTitle',
			defaultTitle: 'Только обход блокировок',
			descKey: 'presets.bypassBlockingDesc',
			defaultDesc:
				'Направляет заблокированные в России ресурсы в VPN, весь остальной трафик идёт напрямую.',
			requiresProxy: true,
			highlights: [
				{
					icon: 'shield',
					labelKey: 'presets.hlAntifilter',
					defaultLabel: 'Блокировки и фильтры',
					target: 'vpn',
				},
				{
					icon: 'alt_route',
					labelKey: 'presets.hlRestDirect',
					defaultLabel: 'Весь остальной трафик',
					target: 'direct',
				},
			],
			tags: [
				{ type: 'geosite', tag: 'antifilter', target: 'vpn' },
				{ type: 'geosite', tag: 'antifilter-community', target: 'vpn' },
				{ type: 'geoip', tag: 'antifilter', target: 'vpn' },
				{ type: 'geoip', tag: 'antifilter-community', target: 'vpn' },
			],
		},
		{
			id: 'russia-direct',
			icon: 'alt_route',
			titleKey: 'presets.russiaDirectTitle',
			defaultTitle: 'Российские ресурсы напрямую',
			descKey: 'presets.russiaDirectDesc',
			defaultDesc:
				'Российские домены, IP-адреса и локальные сети направляются напрямую в обход VPN.',
			requiresProxy: false,
			highlights: [
				{
					icon: 'public',
					labelKey: 'presets.hlRuDirect',
					defaultLabel: 'Российские сервисы',
					target: 'direct',
				},
				{
					icon: 'home',
					labelKey: 'presets.hlPrivateDirect',
					defaultLabel: 'Локальные сети (LAN)',
					target: 'direct',
				},
			],
			tags: [
				{ type: 'geosite', tag: 'category-ru', target: 'direct' },
				{ type: 'geoip', tag: 'ru', target: 'direct' },
				{ type: 'geoip', tag: 'private', target: 'direct' },
			],
		},
	];

	async function handleApply(presetId: string) {
		busyId = presetId;
		try {
			await onApply(presetId);
		} finally {
			busyId = null;
		}
	}

	async function handleRemove(presetId: string) {
		busyId = presetId;
		try {
			await onRemove(presetId);
		} finally {
			busyId = null;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && !busyId) {
			event.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="presets-backdrop"
	role="presentation"
	transition:fade={{ duration: 180 }}
	onclick={(event) => {
		if (event.currentTarget === event.target && !busyId) onClose();
	}}
>
	<div
		class="presets-dialog"
		role="dialog"
		aria-modal="true"
		aria-labelledby="presets-title"
		transition:scale={{ start: 0.96, duration: 260, easing: cubicOut }}
	>
		<!-- Header styled in Lumen ConfirmDialog aesthetic -->
		<div class="presets-header">
			<h2 id="presets-title">{t('rules.presets') || 'Пресеты'}</h2>
			<p class="presets-subtitle">
				{t('presets.subtitle') || 'Сценарии маршрутизации'}
			</p>
		</div>

		<!-- Scrollable Presets List -->
		<div class="presets-list">
			{#each PRESETS as preset (preset.id)}
				{@const isApplied = appliedPresetIds.has(preset.id)}
				{@const disabled = preset.requiresProxy && !hasActiveProxy}
				{@const isExpanded = expandedPresetId === preset.id}

				<div class="preset-card" class:applied={isApplied}>
					<!-- Card Header -->
					<div class="card-head">
						<div class="card-title-group">
							<span class="preset-icon-badge" class:badge-applied={isApplied}>
								<MaterialIcon name={preset.icon} size={18} />
							</span>
							<span class="card-title">
								{t(preset.titleKey) || preset.defaultTitle}
							</span>
						</div>

						{#if isApplied}
							<span class="applied-badge">
								<StatusDot size={6} variant="active" />
								<span>{t('presets.applied') || 'Активен'}</span>
							</span>
						{/if}
					</div>

					<!-- Card Description -->
					<p class="card-desc">
						{t(preset.descKey) || preset.defaultDesc}
					</p>

					<!-- Visual Route Highlights -->
					<div class="highlights-box">
						{#each preset.highlights as hl (hl.labelKey)}
							<div class="highlight-item">
								<div class="highlight-left">
									<MaterialIcon name={hl.icon} size={15} />
									<span class="highlight-label">
										{t(hl.labelKey) || hl.defaultLabel}
									</span>
								</div>
								<span class="target-tag {hl.target}">
									{hl.target === 'vpn' ? 'VPN' : t('rules.direct') || 'Direct'}
								</span>
							</div>
						{/each}
					</div>

					<!-- Expandable details toggle for power users -->
					<div class="details-section">
						<button
							type="button"
							class="details-toggle"
							onclick={() => toggleDetails(preset.id)}
						>
							<span class="details-toggle-text">
								{isExpanded
									? t('presets.hideRules') || 'Скрыть правила'
									: t('presets.showRules', { count: preset.tags.length }) ||
										`Правила (${preset.tags.length})`}
							</span>
							<MaterialIcon
								name={isExpanded ? 'expand_less' : 'expand_more'}
								size={16}
							/>
						</button>

						{#if isExpanded}
							<div class="tags-grid" transition:slide={{ duration: 180 }}>
								{#each preset.tags as ruleTag (ruleTag.type + ':' + ruleTag.tag)}
									<span
										class="rule-chip"
										class:chip-vpn={ruleTag.target === 'vpn'}
										class:chip-direct={ruleTag.target === 'direct'}
									>
										<span class="chip-name">{ruleTag.type}:{ruleTag.tag}</span>
										<span class="chip-arrow">→</span>
										<span class="chip-target">{ruleTag.target.toUpperCase()}</span>
									</span>
								{/each}
							</div>
						{/if}
					</div>

					{#if disabled}
						<div class="disabled-warning">
							<MaterialIcon name="info" size={14} />
							<span>
								{t('presets.requiresProxy') ||
									'Требуется активная подписка или сервер для VPN-правил'}
							</span>
						</div>
					{/if}

					<!-- Action Buttons in ConfirmDialog pill style -->
					<div class="card-actions">
						{#if isApplied}
							<button
								type="button"
								class="card-btn update-btn"
								disabled={busyId !== null || disabled}
								onclick={() => handleApply(preset.id)}
							>
								{#if busyId === preset.id}
									<span class="btn-spinner dark"></span>
								{/if}
								{t('presets.reapply') || 'Обновить'}
							</button>
							<button
								type="button"
								class="card-btn remove-btn"
								disabled={busyId !== null}
								onclick={() => handleRemove(preset.id)}
							>
								{#if busyId === preset.id}
									<span class="btn-spinner danger"></span>
								{/if}
								{t('presets.remove') || 'Удалить'}
							</button>
						{:else}
							<button
								type="button"
								class="card-btn apply-btn"
								disabled={busyId !== null || disabled}
								onclick={() => handleApply(preset.id)}
							>
								{#if busyId === preset.id}
									<span class="btn-spinner dark"></span>
								{/if}
								{t('presets.apply') || 'Применить'}
							</button>
						{/if}
					</div>
				</div>
			{/each}
		</div>

		<!-- Footer in ConfirmDialog style -->
		<div class="presets-footer">
			<button
				bind:this={closeButton}
				type="button"
				class="confirm-button cancel"
				disabled={busyId !== null}
				onclick={onClose}
			>
				{t('common.close') || 'Закрыть'}
			</button>
		</div>
	</div>
</div>

<style>
	.presets-backdrop {
		position: fixed;
		left: 50%;
		top: 50%;
		width: min(var(--app-width), 100vw);
		height: min(var(--app-height), 100dvh);
		transform: translate(-50%, -50%);
		z-index: 80;
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 72px 20px 20px;
		overflow: hidden;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		background: rgba(54, 54, 54, 0.75);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		box-sizing: border-box;
	}

	:global(body:has(.presets-backdrop) .window-header) {
		pointer-events: none;
		opacity: 0.5;
		transition: opacity 180ms ease;
	}

	.presets-dialog {
		width: 100%;
		max-width: 360px;
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 14px;
		color: #fff;
		transform-origin: 50% 50%;
		box-sizing: border-box;
		min-height: 0;
	}

	.presets-header {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		gap: 4px;
		padding: 0 8px;
		flex-shrink: 0;
	}

	h2 {
		margin: 0;
		font-size: 28px;
		font-weight: 500;
		line-height: 1.1;
		letter-spacing: -0.3px;
		color: #ffffff;
	}

	.presets-subtitle {
		margin: 0;
		font-size: 13.5px;
		color: rgba(255, 255, 255, 0.72);
		line-height: 1.35;
	}

	.presets-list {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
		overflow-y: auto;
		padding: 2px 2px 6px;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}

	.presets-list::-webkit-scrollbar {
		display: none;
	}

	.preset-card {
		background: rgba(255, 255, 255, 0.08);
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 24px;
		padding: 16px 16px 14px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		transition:
			background-color 180ms ease,
			border-color 180ms ease,
			transform 180ms var(--ease-spring);
	}

	.preset-card:hover {
		background: rgba(255, 255, 255, 0.11);
		border-color: rgba(255, 255, 255, 0.18);
	}

	.preset-card.applied {
		background: rgba(134, 225, 111, 0.09);
		border-color: rgba(134, 225, 111, 0.38);
		box-shadow: 0 6px 20px rgba(0, 0, 0, 0.2);
	}

	.card-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.card-title-group {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.preset-icon-badge {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.12);
		color: #ffffff;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.preset-icon-badge.badge-applied {
		background: rgba(134, 225, 111, 0.2);
		color: #86e16f;
	}

	.card-title {
		font-size: 15px;
		font-weight: 600;
		color: #ffffff;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.applied-badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 11px;
		font-weight: 600;
		color: #86e16f;
		background: rgba(134, 225, 111, 0.16);
		border-radius: 12px;
		padding: 3px 8px;
		flex-shrink: 0;
	}

	.card-desc {
		margin: 0;
		font-size: 12.5px;
		line-height: 1.42;
		color: rgba(255, 255, 255, 0.72);
	}

	.highlights-box {
		display: flex;
		flex-direction: column;
		gap: 5px;
		background: rgba(0, 0, 0, 0.2);
		border-radius: 16px;
		padding: 8px 10px;
	}

	.highlight-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		font-size: 12px;
	}

	.highlight-left {
		display: flex;
		align-items: center;
		gap: 6px;
		color: rgba(255, 255, 255, 0.88);
		min-width: 0;
	}

	.highlight-label {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.target-tag {
		font-size: 10px;
		font-weight: 600;
		padding: 2px 7px;
		border-radius: 8px;
		text-transform: uppercase;
		letter-spacing: 0.2px;
		flex-shrink: 0;
	}

	.target-tag.vpn {
		background: rgba(100, 180, 255, 0.2);
		color: #70b8ff;
	}

	.target-tag.direct {
		background: rgba(134, 225, 111, 0.18);
		color: #86e16f;
	}

	.details-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.details-toggle {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 4px;
		background: transparent;
		border: none;
		padding: 2px 4px;
		color: rgba(255, 255, 255, 0.55);
		font-size: 11.5px;
		cursor: pointer;
		align-self: flex-start;
		border-radius: 8px;
		transition: color 150ms ease;
	}

	.details-toggle:hover {
		color: rgba(255, 255, 255, 0.85);
	}

	.tags-grid {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
		padding-top: 2px;
	}

	.rule-chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-size: 10px;
		font-family: monospace;
		padding: 2px 7px;
		border-radius: 8px;
		background: rgba(255, 255, 255, 0.08);
		color: rgba(255, 255, 255, 0.85);
	}

	.rule-chip.chip-vpn {
		background: rgba(100, 180, 255, 0.14);
		color: #90cdf4;
	}

	.rule-chip.chip-direct {
		background: rgba(255, 255, 255, 0.08);
		color: rgba(255, 255, 255, 0.7);
	}

	.chip-arrow {
		opacity: 0.5;
	}

	.chip-target {
		font-weight: 600;
	}

	.disabled-warning {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 11.5px;
		color: #ff8585;
		background: rgba(255, 69, 58, 0.12);
		padding: 6px 10px;
		border-radius: 12px;
	}

	.card-actions {
		display: flex;
		gap: 8px;
		margin-top: 2px;
	}

	.card-btn {
		height: 44px;
		border: 0;
		border-radius: 123px;
		font-size: 14px;
		font-weight: 600;
		letter-spacing: -0.2px;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		transition:
			transform 180ms var(--ease-spring),
			background-color 180ms ease,
			opacity 180ms ease;
	}

	.card-btn:hover:not(:disabled) {
		transform: scale(1.02);
	}

	.card-btn:active:not(:disabled) {
		transform: scale(0.98);
	}

	.card-btn.apply-btn {
		width: 100%;
		background: #ffffff;
		color: rgba(0, 0, 0, 0.85);
	}

	.card-btn.apply-btn:hover:not(:disabled) {
		background: #f4f4f4;
	}

	.card-btn.update-btn {
		flex: 1;
		background: #ffffff;
		color: rgba(0, 0, 0, 0.85);
	}

	.card-btn.update-btn:hover:not(:disabled) {
		background: #f4f4f4;
	}

	.card-btn.remove-btn {
		flex: 1;
		background: rgba(255, 69, 58, 0.16);
		color: #ff6b6b;
		border: 1px solid rgba(255, 69, 58, 0.25);
	}

	.card-btn.remove-btn:hover:not(:disabled) {
		background: rgba(255, 69, 58, 0.24);
	}

	.card-btn:disabled {
		opacity: 0.45;
		cursor: not-allowed;
		transform: none;
	}

	.presets-footer {
		flex-shrink: 0;
		width: 100%;
		margin-top: auto;
		padding-top: 4px;
	}

	.confirm-button {
		width: 100%;
		height: 52px;
		min-height: 52px;
		border: 0;
		border-radius: 123px;
		font-size: 16px;
		font-weight: 600;
		letter-spacing: -0.3px;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		transition:
			transform 180ms var(--ease-spring),
			background-color 180ms ease,
			opacity 180ms ease;
	}

	.confirm-button:hover:not(:disabled) {
		transform: scale(1.015);
	}

	.confirm-button:active:not(:disabled) {
		transform: scale(0.985);
	}

	.confirm-button.cancel {
		background: rgba(255, 255, 255, 0.15);
		color: rgba(255, 255, 255, 0.85);
	}

	.confirm-button.cancel:hover {
		background: rgba(255, 255, 255, 0.22);
	}

	.btn-spinner {
		width: 14px;
		height: 14px;
		border: 2px solid rgba(0, 0, 0, 0.2);
		border-top-color: #000000;
		border-radius: 50%;
		animation: spin 0.75s linear infinite;
		display: inline-block;
	}

	.btn-spinner.danger {
		border-color: rgba(255, 69, 58, 0.2);
		border-top-color: #ff6b6b;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
