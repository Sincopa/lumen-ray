<script module lang="ts">
	const persistentRuleIconCache: Record<string, string> = {};
</script>

<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { runtime } from '../../runtime';
	import { getProcesses, getRuleIcons } from '../../services/processes';
	import { createDeferredTask } from '../../utils/deferredTask';
	import { catalog } from '../../services';
	import { errorCode } from '../../services/errors';
	import { t } from '../../i18n';
	import { serverDisplayName } from '../../utils/serverDisplayName';
	import type { Rule } from '../../services/catalog';
	import ConfirmDialog from '../../components/ConfirmDialog.svelte';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import RoundCheckbox from '../../components/RoundCheckbox.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import ServersView from '../servers/ServersView.svelte';
	import RuleEditorView from './RuleEditorView.svelte';
	import ProcessPickerView from './ProcessPickerView.svelte';
	import PresetsModal from './PresetsModal.svelte';
	import searchIcon from '../../../assets/search.svg';
	import reloadIcon from '../../../assets/reload.svg';
	import addIcon from '../../../assets/add.svg';
	import pencilIcon from '../../../assets/pencil.svg';
	import trashIcon from '../../../assets/trash.svg';
	import checkIcon from '../../../assets/check.svg';
	import crossIcon from '../../../assets/cross.svg';
	import chevronIcon from '../../../assets/chevron.svg';
	import discordIcon from '../../../assets/discord.png';
	import yandexIcon from '../../../assets/yandex.png';
	import chromeIcon from '../../../assets/chrome.svg';
	import telegramIcon from '../../../assets/telegram.svg';
	import steamIcon from '../../../assets/steam.svg';

	interface Props {
		onChromeChange?: (mode: 'default' | 'selection' | 'picker') => void;
	}

	let { onChromeChange }: Props = $props();

	const searchTask = createDeferredTask();
	onDestroy(searchTask.cancel);

	const catalogStore = catalog.state;

	// Local state
	let query = $state('');
	let searching = $state(false);
	let searchClosing = $state(false);
	let searchInputEl = $state<HTMLInputElement | null>(null);

	function handleOpenSearch() {
		searching = true;
		searchClosing = false;
		searchTask.schedule(() => {
			searchInputEl?.focus();
		}, 180);
	}

	function handleCloseSearch() {
		searchClosing = true;
		searchTask.schedule(() => {
			searching = false;
			searchClosing = false;
			query = '';
		}, 240);
	}

	let selectMode = $state(false);
	let selectingRule = $state<Rule | null>(null);
	let pendingServerId = $state('');
	let editing = $state<Rule | null>(null);
	let choosingProcess = $state(false);
	let choosingServer = $state(false);
	let deleting = $state(false);
	let selected = $state<string[]>([]);
	let error = $state('');
	let showPresetsModal = $state(false);
	let confirmSwitchToRules = $state(false);

	// Derived state
	const serversById = $derived(
		new Map($catalogStore.servers.map((server) => [server.id, server])),
	);
	const selectedNames = $derived(new Set(selected));
	const activeSubscriptionId = $derived(
		serversById.get($catalogStore.selectedServerId ?? '')?.subscriptionId,
	);
	const activeSub = $derived(
		$catalogStore.subscriptions.find(
			(subscription) => subscription.id === activeSubscriptionId,
		) ?? $catalogStore.subscriptions[0],
	);
	const normalizedQuery = $derived(query.toLowerCase());
	let filtered = $derived(
		normalizedQuery
			? $catalogStore.rules.filter((rule) =>
					rule.name.toLowerCase().includes(normalizedQuery),
				)
			: $catalogStore.rules,
	);

	const appliedPresetIds = $derived(
		new Set(
			$catalogStore.rules
				.map((r) => r.presetId)
				.filter((id): id is string => Boolean(id)),
		),
	);
	const hasActiveProxy = $derived(
		Boolean(
			$catalogStore.servers.length > 0 ||
			$catalogStore.subscriptions.length > 0,
		),
	);

	let processIcons = $state<Record<string, string>>({ ...persistentRuleIconCache });

	function registerIcons(newIcons: Record<string, string>) {
		let changed = false;
		for (const [key, iconVal] of Object.entries(newIcons)) {
			if (!iconVal) continue;
			const lower = key.toLowerCase();
			const stem = lower.replace(/\.exe$/i, '');
			const base = formatRuleDisplayName(key).toLowerCase();
			const baseStem = base.replace(/\.exe$/i, '');

			for (const alias of [lower, stem, base, baseStem]) {
				if (alias && persistentRuleIconCache[alias] !== iconVal) {
					persistentRuleIconCache[alias] = iconVal;
					changed = true;
				}
			}
		}
		if (changed) {
			processIcons = { ...persistentRuleIconCache };
		}
	}

	onMount(() => {
		if (runtime.isTauri) {
			void getProcesses().then((items) => {
				const map: Record<string, string> = {};
				for (const item of items) {
					if (item.icon) {
						map[item.name] = item.icon;
						if (item.detail) map[item.detail] = item.icon;
					}
				}
				registerIcons(map);
			});
		}
	});

	$effect(() => {
		if (!runtime.isTauri) return;
		const appRules = $catalogStore.rules.filter(
			(r) => (r.matcher ?? (r.isDomain ? 'domain' : 'application')) === 'application',
		);
		const needed = new Set<string>();
		for (const r of appRules) {
			const displayName = formatRuleDisplayName(r.name);
			const base = displayName.toLowerCase();
			const stem = base.replace(/\.exe$/i, '');
			const full = r.name.toLowerCase();
			if (
				!processIcons[base] &&
				!processIcons[stem] &&
				!processIcons[full] &&
				!persistentRuleIconCache[base] &&
				!persistentRuleIconCache[stem] &&
				!persistentRuleIconCache[full]
			) {
				needed.add(r.name);
				needed.add(displayName);
			}
		}
		if (needed.size > 0) {
			void getRuleIcons(Array.from(needed)).then((icons) => {
				registerIcons(icons);
			});
		}
	});

	function formatRuleDisplayName(name: string): string {
		if (name.includes('\\') || name.includes('/')) {
			const parts = name.split(/[\\/]/);
			return parts[parts.length - 1] || name;
		}
		return name;
	}

	function getRuleIcon(rule: Rule): string | null {
		const matcher = rule.matcher ?? (rule.isDomain ? 'domain' : 'application');
		if (matcher !== 'application') return null;
		const baseName = formatRuleDisplayName(rule.name).toLowerCase();
		const stemName = baseName.replace(/\.exe$/i, '');
		const fullName = rule.name.toLowerCase();

		if (processIcons[baseName]) return processIcons[baseName];
		if (processIcons[stemName]) return processIcons[stemName];
		if (processIcons[fullName]) return processIcons[fullName];
		if (persistentRuleIconCache[baseName]) return persistentRuleIconCache[baseName];
		if (persistentRuleIconCache[stemName]) return persistentRuleIconCache[stemName];
		if (persistentRuleIconCache[fullName]) return persistentRuleIconCache[fullName];

		if (baseName.includes('chrome')) return chromeIcon;
		if (baseName.includes('discord')) return discordIcon;
		if (baseName.includes('yandex')) return yandexIcon;
		if (baseName.includes('telegram')) return telegramIcon;
		if (baseName.includes('steam')) return steamIcon;
		return null;
	}

	$effect(() => {
		onChromeChange?.(
			selectingRule || editing
				? 'picker'
				: selectMode
					? 'selection'
					: 'default',
		);
	});

	// Action handlers
	async function refresh() {
		try {
			if (activeSub) {
				await catalog.refreshSubscription(activeSub.id);
			}
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	async function pickServer(server: { id: string }) {
		if (!selectingRule) return;
		pendingServerId = server.id;
		try {
			await catalog.saveRule({ ...selectingRule, serverId: server.id });
			selectingRule = null;
			pendingServerId = '';
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function confirmServer() {
		if (!selectingRule || !pendingServerId) return;
		pickServer({ id: pendingServerId });
	}

	function add() {
		error = '';
		editing = {
			id: crypto.randomUUID(),
			name: '',
			matcher: 'domain',
			isDomain: true,
			serverId: $catalogStore.selectedServerId || 'subscription',
			enabled: true,
		};
	}

	async function handleApplyPreset(presetId: string) {
		try {
			await catalog.applyPreset(presetId);
			if ($catalogStore.routingMode === 'global') {
				showPresetsModal = false;
				confirmSwitchToRules = true;
			}
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	async function handleRemovePreset(presetId: string) {
		try {
			await catalog.removePreset(presetId);
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	async function toggle(rule: Rule) {
		try {
			await catalog.saveRule({ ...rule, enabled: !rule.enabled });
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	async function remove() {
		try {
			await catalog.removeRules(new Set(selected));
			selected = [];
			deleting = false;
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}
</script>

{#if editing}
	{#if choosingProcess}
		<ProcessPickerView
			selectedProcess={editing.name}
			onSelect={(procs: string[]) => {
				if (editing) {
					editing.name = procs.join(', ');
					editing.matcher = 'application';
					editing.isDomain = false;
				}
				choosingProcess = false;
			}}
			onCancel={() => {
				choosingProcess = false;
			}}
		/>
	{:else if choosingServer}
		<div class="server-picker-view">
			<ServersView
				selectedId={editing.serverId}
				onSelect={(server) => {
					if (editing) editing.serverId = server.id;
					choosingServer = false;
				}}
				compact
			/>
			<div class="picker-bar">
				<IconButton
					icon={chevronIcon}
					ariaLabel={t('common.close')}
					size="lg"
					variant="surface"
					class="back-round"
					onclick={() => {
						choosingServer = false;
					}}
				/>
				<IconButton
					icon={checkIcon}
					ariaLabel={t('common.save')}
					size="lg"
					variant="primary"
					onclick={() => {
						choosingServer = false;
					}}
				/>
			</div>
		</div>
	{:else}
		{@const currentServer = serversById.get(editing.serverId)}
		<RuleEditorView
			rule={editing}
			server={currentServer}
			subscriptionName={activeSub?.name}
			onSave={async (updated: Rule | Rule[]) => {
				try {
					if (Array.isArray(updated)) {
						await catalog.saveRules(updated);
					} else {
						await catalog.saveRule(updated);
					}
					editing = null;
					choosingProcess = false;
					choosingServer = false;
					error = '';
				} catch (e) {
					error = t('errors.' + errorCode(e));
				}
			}}
			onCancel={() => {
				editing = null;
				choosingProcess = false;
				choosingServer = false;
			}}
			onSelectProcess={() => {
				choosingProcess = true;
			}}
			onSelectServer={() => {
				choosingServer = true;
			}}
			onMatcherChange={(matcher) => {
				if (editing) {
					editing.matcher = matcher;
					editing.isDomain = matcher === 'domain';
				}
			}}
		/>
	{/if}
{:else if selectingRule}
	<div class="server-picker-view">
		<ServersView selectedId={pendingServerId} onSelect={pickServer} compact />
		<div class="picker-bar">
			<IconButton
				icon={chevronIcon}
				ariaLabel={t('common.close')}
				size="lg"
				variant="surface"
				class="back-round"
				onclick={() => {
					selectingRule = null;
					pendingServerId = '';
				}}
			/>
			<IconButton
				icon={checkIcon}
				ariaLabel={t('common.save')}
				size="lg"
				variant="primary"
				onclick={confirmServer}
			/>
		</div>
	</div>
{:else}
	<section class="rules-view" aria-label={t('rules.title')}>
		{#if selectMode}
			<div class="selection-header">
				<div class="sub-info">
					<span class="sub-title">{activeSub?.name ?? t('rules.title')}</span>
					<StatusDot />
				</div>
				<button
					class="select-all"
					role="checkbox"
					aria-checked={selected.length === filtered.length &&
						filtered.length > 0}
					onclick={() =>
						(selected =
							selected.length === filtered.length
								? []
								: filtered.map((rule) => rule.id))}
				>
					<span>{t('rules.selectAll')}</span>
					<RoundCheckbox
						checked={selected.length === filtered.length && filtered.length > 0}
						asSpan
					/>
				</button>
			</div>
		{:else}
			<div class="header-card-wrapper">
				<div class="header-row">
					<div class="header-pill">
						<!-- Normal header content -->
						<div class="header-normal" class:header-hidden={searching}>
							<div class="header-card-title">
								<span class="title-text">
									{activeSub?.name ?? t('rules.title')}
								</span>
								<StatusDot variant="active" />
							</div>
							<div class="header-card-actions">
								<IconButton
									icon={searchIcon}
									ariaLabel={t('common.search')}
									onclick={handleOpenSearch}
								/>
								<IconButton
									icon={reloadIcon}
									ariaLabel={t('common.reload')}
									onclick={refresh}
								/>
								<IconButton
									ariaLabel={t('rules.presets') || 'Пресеты'}
									title={t('rules.presets') || 'Пресеты'}
									onclick={() => (showPresetsModal = true)}
								>
									<MaterialIcon name="tune" size={18} />
								</IconButton>
								<IconButton
									icon={pencilIcon}
									ariaLabel={t('rules.selectMode')}
									onclick={() => (selectMode = true)}
								/>
							</div>
						</div>

						<!-- Search bar inside header pill (identical to ServersView) -->
						<div
							class="header-search"
							class:search-open={searching && !searchClosing}
							class:search-closing={searchClosing}
						>
							<img src={searchIcon} alt="" class="search-header-icon" />
							<input
								bind:this={searchInputEl}
								type="text"
								class="search-header-input"
								placeholder={t('rules.searchPlaceholder') || 'Поиск правил...'}
								bind:value={query}
								autocomplete="off"
								spellcheck="false"
							/>
							<button
								type="button"
								class="close-search-btn"
								onclick={handleCloseSearch}
								aria-label={t('common.close')}
							>
								<img src={crossIcon} alt="" class="close-icon" />
							</button>
						</div>
					</div>

					<IconButton
						icon={addIcon}
						ariaLabel={t('common.add')}
						size="lg"
						variant="surface"
						class="add-rule-btn"
						disabled={!$catalogStore.servers.length}
						onclick={add}
					/>
				</div>
			</div>
		{/if}
		{#if error}<p role="alert">{error}</p>{/if}
		<div class="rules-list">
			{#each filtered as rule (rule.id)}
				{@const server = serversById.get(rule.serverId)}
				{@const isDirect = rule.serverId === 'direct'}
				{@const isBlock = rule.serverId === 'block'}
				{@const isSub =
					rule.serverId === 'subscription' ||
					(activeSub && rule.serverId === activeSub.id)}
				{@const serverName = isDirect
					? t('rules.direct')
					: isBlock
						? t('rules.block') || 'Блокировать'
						: isSub
							? (activeSub?.name ?? t('rules.subscriptionServer'))
							: server
								? serverDisplayName(server)
								: '—'}
				{@const matcher =
					rule.matcher ?? (rule.isDomain ? 'domain' : 'application')}
				{@const isGeo = matcher === 'geosite' || matcher === 'geoip'}
				{@const ruleIcon = getRuleIcon(rule)}
				<article class="rule-row">
					<div class="rule-left">
						<div class="app-icon-wrapper" class:domain-icon={!ruleIcon}>
							{#if ruleIcon}
								<img class="app-icon" src={ruleIcon} alt="" />
							{:else if matcher === 'geosite'}
								<span class="globe-text" aria-hidden="true">
									<MaterialIcon name="travel_explore" size={20} />
								</span>
							{:else if matcher === 'geoip'}
								<span class="globe-text" aria-hidden="true">
									<MaterialIcon name="pin_drop" size={20} />
								</span>
							{:else}
								<span class="globe-text" aria-hidden="true">
									<MaterialIcon
										name={rule.isDomain || matcher === 'domain'
											? 'public'
											: 'description'}
										size={20}
									/>
								</span>
							{/if}
						</div>
						<button
							class="rule-text"
							onclick={() => {
								error = '';
								editing = { ...rule };
							}}
							aria-label={t('rules.edit', { name: rule.name })}
						>
							<span class="rule-name">{formatRuleDisplayName(rule.name)}</span>
							<span class="rule-details">
								{isGeo
									? matcher === 'geosite'
										? 'GeoSite'
										: 'GeoIP'
									: serverName}
							</span>
						</button>
					</div>
					<div class="rule-right">
						<button
							class="server-pill"
							onclick={() => {
								selectingRule = rule;
								pendingServerId = rule.serverId;
							}}
							aria-label={rule.name + ': ' + t('servers.title')}
						>
							{#if isDirect}
								<span class="flag-sm">
									<MaterialIcon name="alt_route" size={16} />
								</span>
							{:else if isBlock}
								<span class="flag-sm">
									<MaterialIcon name="block" size={16} />
								</span>
							{:else if isSub}
								<span class="flag-sm">
									<MaterialIcon name="sync_alt" size={16} />
								</span>
							{:else if server}
								<span class="flag-sm">
									<CountryFlag countryCode={server.countryCode} size={20} />
								</span>
							{/if}
							<span class="server-pill-name">{serverName}</span>
						</button>
						{#if selectMode}
							<RoundCheckbox
								checked={selectedNames.has(rule.id)}
								ariaLabel={t('rules.select', { name: rule.name })}
								onclick={() =>
									(selected = selectedNames.has(rule.id)
										? selected.filter((id) => id !== rule.id)
										: [...selected, rule.id])}
							/>
						{:else}
							<ToggleSwitch
								checked={rule.enabled}
								mode="switch"
								ariaLabel={t('rules.toggle', { name: rule.name })}
								onchange={() => toggle(rule)}
							/>
						{/if}
					</div>
				</article>
			{:else}
				<div class="empty-rules-container">
					<p class="rules-empty">{t('rules.empty')}</p>
					<button
						type="button"
						class="btn-choose-preset"
						onclick={() => (showPresetsModal = true)}
					>
						<MaterialIcon name="tune" size={16} />
						<span>{t('rules.presets') || 'Пресеты'}</span>
					</button>
				</div>
			{/each}
		</div>
		{#if selectMode}
			<div class="select-bar">
				<IconButton
					icon={trashIcon}
					ariaLabel={t('rules.deleteSelected')}
					size="lg"
					variant="danger"
					disabled={!selected.length}
					onclick={() => (deleting = true)}
				/>
				<IconButton
					icon={checkIcon}
					ariaLabel={t('common.close')}
					size="lg"
					variant="primary"
					onclick={() => {
						selectMode = false;
						selected = [];
					}}
				/>
			</div>
		{/if}
	</section>
{/if}

{#if deleting}
	<ConfirmDialog
		title={t('common.areYouSure')}
		description={t('rules.deleteConfirm', { count: selected.length })}
		cancelLabel={t('common.cancel')}
		confirmLabel={t('common.continue')}
		onCancel={() => (deleting = false)}
		onConfirm={remove}
	/>
{/if}

{#if showPresetsModal}
	<PresetsModal
		{appliedPresetIds}
		{hasActiveProxy}
		onApply={handleApplyPreset}
		onRemove={handleRemovePreset}
		onClose={() => (showPresetsModal = false)}
	/>
{/if}

{#if confirmSwitchToRules}
	<ConfirmDialog
		title={t('presets.switchModeTitle') || 'Включить режим правил?'}
		description={t('presets.switchModeDesc') ||
			'Пресет применен. Для его работы рекомендуется переключить режим приложения на «Правила».'}
		cancelLabel={t('common.cancel')}
		confirmLabel={t('presets.switchToRules') || 'Включить'}
		onCancel={() => (confirmSwitchToRules = false)}
		onConfirm={async () => {
			await catalog.setRoutingMode('rules');
			confirmSwitchToRules = false;
		}}
	/>
{/if}

<style>
	.empty-rules-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		padding: 40px 16px;
	}

	.btn-choose-preset {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		background: rgba(255, 255, 255, 0.08);
		border: 1px solid rgba(255, 255, 255, 0.14);
		border-radius: 16px;
		padding: 8px 16px;
		color: #ffffff;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition:
			background 180ms ease,
			border-color 180ms ease;
	}

	.btn-choose-preset:hover {
		background: rgba(255, 255, 255, 0.16);
		border-color: rgba(255, 255, 255, 0.25);
	}

	.rules-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 10px;
		padding-top: 0;
		position: relative;
		min-height: 0;
		height: 100%;
	}
	.server-picker-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 10px;
		overflow: hidden;
	}
	.picker-bar {
		display: flex;
		width: 100%;
		justify-content: space-between;
		align-items: center;
		flex-shrink: 0;
		padding: 0 4px;
		margin-bottom: 33px;
	}
	:global(.back-round .btn-icon) {
		transform: rotate(90deg) !important;
	}
	.header-card-wrapper {
		position: relative;
		width: 100%;
		flex-shrink: 0;
	}
	.header-row {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		box-sizing: border-box;
		padding: 2px 2px 0 0;
	}
	.header-pill {
		position: relative;
		height: 62px;
		background: #00000038;
		border-radius: 124px;
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		box-sizing: border-box;
		flex: 1;
		min-width: 0;
		overflow: hidden;
		border: 1px solid transparent;
		transition: border-color 180ms ease;
	}

	.header-pill:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
	}

	.header-normal {
		width: 100%;
		height: 100%;
		padding: 0 12px 0 24px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		box-sizing: border-box;
		gap: 8px;
		transition:
			opacity 200ms ease,
			transform 200ms ease;
	}

	.header-normal.header-hidden {
		opacity: 0;
		pointer-events: none;
		transform: scale(0.96);
	}

	.header-card-title {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		font-size: 15px;
		font-weight: 500;
		letter-spacing: -0.2px;
	}

	.title-text {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.header-card-actions {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-shrink: 0;
	}

	/* Header Search Bar */
	.header-search {
		position: absolute;
		left: 0;
		top: 0;
		width: 100%;
		height: 100%;
		padding: 0 8px 0 20px;
		display: flex;
		align-items: center;
		gap: 12px;
		box-sizing: border-box;
		opacity: 0;
		pointer-events: none;
		transform: scale(0.96);
		transition:
			opacity 220ms ease,
			transform 220ms cubic-bezier(0.22, 1, 0.36, 1);
	}

	.header-search.search-open {
		opacity: 1;
		pointer-events: auto;
		transform: scale(1);
	}

	.header-search.search-closing {
		opacity: 0;
		pointer-events: none;
		transform: scale(0.96);
	}

	.search-header-icon {
		width: 18px;
		height: 18px;
		object-fit: contain;
		filter: brightness(0) invert(1);
		opacity: 0.6;
		flex-shrink: 0;
	}

	.search-header-input {
		flex: 1;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		border-radius: 0 !important;
		color: #ffffff;
		font-size: 14px;
		font-family: inherit;
		padding: 0 !important;
		-webkit-appearance: none;
		appearance: none;
		min-width: 0;
	}

	.search-header-input::placeholder {
		color: rgba(255, 255, 255, 0.4);
	}

	.close-search-btn {
		width: 36px;
		height: 36px;
		border-radius: 50%;
		background: #ffffff;
		border: none;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		transition: transform 160ms var(--ease-spring);
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
	}

	.close-search-btn:hover {
		transform: scale(1.06);
	}

	.close-search-btn:active {
		transform: scale(0.94);
	}

	.close-icon {
		width: 14px;
		height: 14px;
		object-fit: contain;
		filter: brightness(0);
	}

	:global(.add-rule-btn) {
		flex-shrink: 0;
		transform-origin: 80% 50% !important;
	}
	.selection-header {
		height: 40px;
		min-height: 40px;
		padding: 0 4px 0 12px;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.selection-header + .rules-list {
		margin-top: 6px;
	}
	.select-all {
		min-height: 44px;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 0;
		border: 0;
		background: transparent;
		font-size: 12px;
	}
	.sub-info {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}
	.sub-title {
		font-size: 15px;
		font-weight: 500;
		letter-spacing: -0.2px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.rules-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		width: 100%;
		overflow-y: auto;
		flex: 1;
		min-height: 0;
		padding-bottom: 96px;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}
	.rules-list::-webkit-scrollbar {
		display: none;
	}
	.rules-empty {
		margin: 14px auto 0;
		color: rgba(255, 255, 255, 0.25);
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 400;
		line-height: 1;
		letter-spacing: -0.28px;
		text-align: center;
	}
	.rule-row {
		min-height: 60px;
		border-radius: 124px;
		padding: 0 14px 0 10px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		gap: 8px;
	}
	.rule-row:hover {
		background: #ffffff0a;
	}
	.rule-left {
		display: flex;
		align-items: center;
		gap: 12px;
		min-width: 0;
	}
	.app-icon-wrapper {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		overflow: hidden;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.domain-icon {
		background: #ffffff1a;
		border-radius: 50%;
	}
	.app-icon {
		width: 36px;
		height: 36px;
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
	}
	.rule-name {
		font-size: 14px;
		font-weight: 500;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.rule-details {
		font-size: 12px;
		color: #ffffff80;
	}
	.rule-right {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-shrink: 0;
	}
	.server-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 32px;
		min-height: 0;
		background: #00000047;
		border: none;
		outline: none;
		box-shadow: none;
		border-radius: 124px;
		padding: 4px 12px 4px 6px;
	}
	.server-pill:hover {
		background: #00000073;
	}
	.flag-sm {
		width: 20px;
		height: 20px;
		border-radius: 50%;
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.server-pill-name {
		font-size: 12px;
		font-weight: 500;
		color: #ffffffe6;
		white-space: nowrap;
		max-width: 80px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.select-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		padding: 4px 0 0;
		margin-bottom: 33px;
		flex-shrink: 0;
	}
</style>
