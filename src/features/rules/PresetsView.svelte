<script lang="ts">
	import { t } from '../../lib/i18n';
	import { reveal, pop, rise } from '../../lib/motion';
	import IconButton from '../../components/IconButton.svelte';
	import backIcon from '../../assets/back.svg';
	import chevronIcon from '../../assets/chevron-up.svg';

	interface Props {
		appliedPresetIds: Set<string>;
		hasActiveProxy: boolean;
		onApply: (presetId: string) => Promise<void>;
		onRemove: (presetId: string) => Promise<void>;
		onClose: () => void;
	}

	let { appliedPresetIds, hasActiveProxy, onApply, onRemove, onClose }: Props =
		$props();

	interface PresetRoute {
		labelKey: string;
		target: 'vpn' | 'direct';
	}

	interface PresetDef {
		id: string;
		titleKey: string;
		descKey: string;
		requiresProxy: boolean;
		routes: PresetRoute[];
	}

	// Rule tags for each preset live in catalog.mergePreset; these are the summaries.
	const PRESETS: PresetDef[] = [
		{
			id: 'bypass-blocking',
			titleKey: 'presets.bypassBlockingTitle',
			descKey: 'presets.bypassBlockingDesc',
			requiresProxy: true,
			routes: [
				{ labelKey: 'presets.hlAntifilter', target: 'vpn' },
				{ labelKey: 'presets.hlRestDirect', target: 'direct' },
			],
		},
		{
			id: 'russia-optimized',
			titleKey: 'presets.russiaOptimizedTitle',
			descKey: 'presets.russiaOptimizedDesc',
			requiresProxy: true,
			routes: [
				{ labelKey: 'presets.hlAntifilter', target: 'vpn' },
				{ labelKey: 'presets.hlRuDirect', target: 'direct' },
				{ labelKey: 'presets.hlPrivateDirect', target: 'direct' },
			],
		},
		{
			id: 'russia-direct',
			titleKey: 'presets.russiaDirectTitle',
			descKey: 'presets.russiaDirectDesc',
			requiresProxy: false,
			routes: [
				{ labelKey: 'presets.hlRuDirect', target: 'direct' },
				{ labelKey: 'presets.hlPrivateDirect', target: 'direct' },
			],
		},
	];

	// Presets are a single choice: the radio starts on the one already applied.
	const initialId =
		PRESETS.find((preset) => appliedPresetIds.has(preset.id))?.id ?? null;
	let selectedId = $state<string | null>(initialId);
	let expandedId = $state<string | null>(initialId);
	let busy = $state(false);

	const canContinue = $derived(
		!busy && (selectedId !== null || appliedPresetIds.size > 0),
	);

	function isDisabled(preset: PresetDef) {
		return preset.requiresProxy && !hasActiveProxy;
	}

	function choose(preset: PresetDef) {
		if (busy) return;
		expandedId = preset.id;
		if (isDisabled(preset)) return;
		selectedId = selectedId === preset.id ? null : preset.id;
	}

	async function submit() {
		if (!canContinue) return;
		busy = true;
		try {
			for (const id of appliedPresetIds) {
				if (id !== selectedId) await onRemove(id);
			}
			if (selectedId) await onApply(selectedId);
		} finally {
			busy = false;
		}
		onClose();
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && !busy) {
			event.preventDefault();
			onClose();
		}
	}}
/>

<section class="presets-view" aria-label={t('rules.presets')}>
	<div class="preset-list" role="radiogroup" aria-label={t('rules.presets')}>
		{#each PRESETS as preset, index (preset.id)}
			{@const selected = selectedId === preset.id}
			{@const expanded = expandedId === preset.id}
			{@const disabled = isDisabled(preset)}
			<article
				class="preset-card"
				class:open={selected || expanded}
				class:disabled
				in:rise|global={{ y: 12, delay: 40 * index, duration: 260 }}
			>
				<button
					type="button"
					class="preset-main"
					role="radio"
					aria-checked={selected}
					aria-disabled={disabled}
					onclick={() => choose(preset)}
				>
					<span class="preset-head">
						<span class="preset-title">{t(preset.titleKey)}</span>
						<span class="preset-radio" aria-hidden="true">
							{#if selected}<span class="preset-dot" transition:pop></span>{/if}
						</span>
					</span>
					<span class="preset-desc">{t(preset.descKey)}</span>
				</button>
				<IconButton
					icon={chevronIcon}
					ariaLabel={t(preset.titleKey)}
					{expanded}
					class="preset-chevron"
					onclick={() => (expandedId = expanded ? null : preset.id)}
				/>
				{#if expanded}
					<div class="preset-routes" transition:reveal>
						{#each preset.routes as route (route.labelKey)}
							<span class="route-chip">
								{t(route.labelKey)}
								<span class="route-target {route.target}">
									{route.target === 'vpn' ? 'VPN' : t('rules.direct')}
								</span>
							</span>
						{/each}
						{#if disabled}
							<p class="preset-warning">{t('presets.requiresProxy')}</p>
						{/if}
					</div>
				{/if}
			</article>
		{/each}
	</div>

	<div class="design-action-bar">
		<IconButton
			design
			icon={backIcon}
			ariaLabel={t('common.back')}
			size="lg"
			variant="surface"
			disabled={busy}
			onclick={onClose}
		/>
		<button
			type="button"
			class="continue-btn"
			disabled={!canContinue}
			aria-busy={busy}
			onclick={submit}
		>
			<span>{t('common.continue')}</span>
			<svg
				width="34"
				height="34"
				viewBox="0 0 34 34"
				fill="none"
				aria-hidden="true"
			>
				<path
					d="m13.25 9.5 7.1 7.1a.6.6 0 0 1 0 .8l-7.1 7.1"
					stroke="currentColor"
					stroke-width="1.7"
					stroke-linecap="round"
				/>
			</svg>
		</button>
	</div>
</section>

<style>
	/* Preset radio list with a back and continue bar. */
	.presets-view {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		height: 100%;
		min-height: 0;
		box-sizing: border-box;
	}
	.preset-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
		padding-bottom: 96px;
		overflow-y: auto;
		scrollbar-width: none;
	}
	.preset-card {
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		width: 100%;
		padding-bottom: 18px;
		border-radius: 24px;
		background: var(--design-panel);
		color: #fff;
		transition: background-color 220ms ease;
	}
	.preset-card.open {
		background: var(--design-panel-open);
	}
	.preset-main {
		display: flex;
		flex-direction: column;
		gap: 10px;
		width: 100%;
		padding: 16px 16px 8px 24px;
		border: 0;
		border-radius: 24px 24px 0 0;
		background: transparent;
		color: inherit;
		text-align: left;
		transition: transform 160ms var(--ease-out);
	}
	.preset-main:active {
		transform: scale(0.99);
	}
	.preset-card.disabled .preset-main {
		cursor: not-allowed;
	}
	.preset-card.disabled .preset-title,
	.preset-card.disabled .preset-radio {
		opacity: 0.45;
	}
	.preset-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 28px;
	}
	.preset-title {
		font-size: 16px;
		font-weight: 500;
		line-height: 20.8px;
		letter-spacing: -0.32px;
	}
	.preset-radio {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: #ffffff0a;
		-webkit-backdrop-filter: blur(12.5px);
		backdrop-filter: blur(12.5px);
	}
	.preset-dot {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 2px 7.7px #00000040;
	}
	.preset-desc {
		width: 290px;
		max-width: calc(100% - 38px);
		color: #ffffff80;
		font-size: 16px;
		line-height: 20px;
		letter-spacing: 0.16px;
	}
	.preset-card :global(.preset-chevron) {
		position: absolute;
		top: 55px;
		right: 16px;
		width: 28px;
		height: 28px;
		background: transparent;
		transform: rotate(180deg);
		transition: transform 240ms var(--ease-out);
	}
	.preset-card :global(.preset-chevron.expanded) {
		transform: rotate(0);
	}
	.preset-routes {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 10px;
		padding: 0 18px;
	}
	.route-chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 34px;
		padding: 6px 6px 6px 14px;
		border-radius: 123px;
		background: #ffffff0d;
		box-sizing: border-box;
		font-size: 16px;
		line-height: 20px;
		letter-spacing: 0.16px;
		white-space: nowrap;
	}
	.route-target {
		display: inline-flex;
		align-items: center;
		height: 22px;
		padding: 0 8px;
		border-radius: 123px;
		font-size: 14px;
		font-weight: 600;
		line-height: 17.5px;
		letter-spacing: 0.14px;
	}
	.route-target.vpn {
		background: #61d89840;
		color: #61d898;
	}
	.route-target.direct {
		background: #61aed840;
		color: #61aed8;
	}
	.preset-warning {
		width: 100%;
		margin: 4px 6px 0;
		color: #ff8585;
		font-size: 13px;
		line-height: 17px;
	}
	.continue-btn {
		display: flex;
		justify-content: space-between;
		align-items: center;
		flex: 1;
		max-width: 272px;
		height: 50px;
		padding: 8px 8px 8px 24px;
		border: 0;
		border-radius: 123px;
		background: #fff;
		color: #262626;
		font-size: 16px;
		font-weight: 500;
		line-height: 20.8px;
		transition:
			background-color 180ms ease,
			color 180ms ease,
			transform 160ms var(--ease-out);
	}
	.continue-btn:disabled {
		background: #262626;
		color: #ffffff4d;
		opacity: 1;
	}
	.continue-btn:active:not(:disabled) {
		transform: scale(0.97);
	}
	.continue-btn[aria-busy='true'] {
		opacity: 0.7;
	}
</style>
