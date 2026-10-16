<script module lang="ts">
	const persistentRuleIconCache: Record<string, string> = {};
</script>

<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { runtime } from '../../lib/runtime';
	import { getProcesses, getRuleIcons } from '../../lib/services/processes';
	import { createDeferredTask } from '../../lib/utils/deferredTask';
	import { catalog } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { t } from '../../lib/i18n';
	import { serverDisplayName } from '../../lib/utils/serverDisplayName';
	import type { Rule } from '../../lib/services/catalog';
	import ConfirmDialog from '../../components/ConfirmDialog.svelte';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import PopupMenu from '../../components/PopupMenu.svelte';
	import MenuItem from '../../components/MenuItem.svelte';
	import backIcon from '../../assets/back.svg';
	import menuIcon from '../../assets/menu.svg';
	import tunIcon from '../../assets/mode-rules.svg';
	import StatusDot from '../../components/StatusDot.svelte';
	import RoundCheckbox from '../../components/RoundCheckbox.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import ServersView from '../servers/ServersView.svelte';
	import RuleEditorView from './RuleEditorView.svelte';
	import ProcessPickerView from './ProcessPickerView.svelte';
	import PresetsView from './PresetsView.svelte';
	import { flip } from 'svelte/animate';
	import { screen, rise, pop, easeOut } from '../../lib/motion';
	import searchIcon from '../../assets/search.svg';
	import reloadIcon from '../../assets/reload.svg';
	import addIcon from '../../assets/plus.svg';
	import pencilIcon from '../../assets/pencil.svg';
	import trashIcon from '../../assets/trash.svg';
	import checkIcon from '../../assets/check.svg';
	import crossIcon from '../../assets/close.svg';
	import yandexIcon from '../../assets/yandex.png';

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
	let pickerSearchSignal = $state(0);
	let editing = $state<Rule | null>(null);
	let editorStep = $state<'type' | 'form'>('form');
	let choosingProcess = $state(false);
	let choosingServer = $state(false);
	let deleting = $state(false);
	let selected = $state<string[]>([]);
	let error = $state('');
	let showPresets = $state(false);
	let menu = $state(false);
	let confirmSwitchToRules = $state(false);

	// Derived state
	const serversById = $derived(
		new Map($catalogStore.servers.map((server) => [server.id, server])),
	);
	const subscriptionsById = $derived(
		new Map(
			$catalogStore.subscriptions.map((subscription) => [
				subscription.id,
				subscription,
			]),
		),
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

	let processIcons = $state<Record<string, string>>({
		...persistentRuleIconCache,
	});

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
			(r) =>
				(r.matcher ?? (r.isDomain ? 'domain' : 'application')) ===
				'application',
		);
		// Local deduplication scratch space; it is never exposed as reactive state.
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
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
		if (persistentRuleIconCache[baseName])
			return persistentRuleIconCache[baseName];
		if (persistentRuleIconCache[stemName])
			return persistentRuleIconCache[stemName];
		if (persistentRuleIconCache[fullName])
			return persistentRuleIconCache[fullName];

		if (baseName.includes('yandex')) return yandexIcon;
		return null;
	}

	$effect(() => {
		onChromeChange?.(
			selectingRule || editing || showPresets
				? 'picker'
				: selectMode
					? 'selection'
					: 'default',
		);
	});

	// Sub-screens push in from the side they logically live on.
	const screenDepth: Record<string, number> = {
		list: 0,
		presets: 1,
		pick: 1,
		editor: 1,
		server: 2,
		process: 2,
	};
	const rulesScreen = $derived(
		editing
			? choosingProcess
				? 'process'
				: choosingServer
					? 'server'
					: 'editor'
			: selectingRule
				? 'pick'
				: showPresets
					? 'presets'
					: 'list',
	);
	let lastScreen = 'list';
	function screenDir(next: string) {
		const dir =
			(screenDepth[next] ?? 0) >= (screenDepth[lastScreen] ?? 0) ? 1 : -1;
		lastScreen = next;
		return dir;
	}

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
		editorStep = 'type';
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
				showPresets = false;
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

<svelte:window
	onclick={(event) => {
		const target = event.target as HTMLElement | null;
		if (!target?.closest('.popup-menu, .icon-btn')) menu = false;
	}}
	onkeydown={(event) => {
		if (event.key === 'Escape') {
			menu = false;
			if (searching) handleCloseSearch();
		}
	}}
/>

{#key rulesScreen}
	<div class="rules-screen" in:screen={{ dir: screenDir(rulesScreen) }}>
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
						searchSignal={pickerSearchSignal}
						selectedId={editing.serverId}
						onSelect={(server) => {
							if (editing) editing.serverId = server.id;
							choosingServer = false;
						}}
						compact
					/>
					<div class="design-action-bar picker-bar">
						<IconButton
							design
							icon={backIcon}
							ariaLabel={t('common.back')}
							size="lg"
							variant="surface"
							onclick={() => {
								choosingServer = false;
							}}
						/>
						<IconButton
							design
							icon={searchIcon}
							ariaLabel={t('common.search')}
							size="lg"
							variant="surface"
							onclick={() => pickerSearchSignal++}
						/>
						<IconButton
							design
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
					step={editorStep}
					onStepChange={(next) => (editorStep = next)}
					onNameChange={(name) => {
						if (editing) editing.name = name;
					}}
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
						pickerSearchSignal = 0;
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
				<ServersView
					selectedId={pendingServerId}
					searchSignal={pickerSearchSignal}
					onSelect={pickServer}
					compact
				/>
				<div class="design-action-bar picker-bar">
					<IconButton
						design
						icon={searchIcon}
						ariaLabel={t('common.search')}
						size="lg"
						variant="surface"
						onclick={() => pickerSearchSignal++}
					/>
					<IconButton
						design
						icon={checkIcon}
						ariaLabel={t('common.save')}
						size="lg"
						variant="primary"
						onclick={confirmServer}
					/>
				</div>
			</div>
		{:else if showPresets}
			<PresetsView
				{appliedPresetIds}
				{hasActiveProxy}
				onApply={handleApplyPreset}
				onRemove={handleRemovePreset}
				onClose={() => (showPresets = false)}
			/>
		{:else}
			<section
				class="rules-view"
				class:empty={!$catalogStore.rules.length}
				aria-label={t('rules.title')}
			>
				{#if selectMode}
					<!-- Selection replaces the header with "Select all". -->
					<div class="select-all-row" transition:rise={{ y: -8 }}>
						<button
							type="button"
							class="select-all-btn"
							onclick={() =>
								(selected =
									selected.length === filtered.length
										? []
										: filtered.map((rule) => rule.id))}
						>
							<span>Select all</span>
							<RoundCheckbox
								asSpan
								checked={filtered.length > 0 &&
									selected.length === filtered.length}
							/>
						</button>
					</div>
				{/if}
				<div
					class="header-card-wrapper"
					class:selection={selectMode}
					hidden={selectMode}
				>
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
										design
										icon={searchIcon}
										ariaLabel={t('common.search')}
										onclick={handleOpenSearch}
									/>
									<IconButton
										design
										icon={reloadIcon}
										ariaLabel={t('common.reload')}
										onclick={refresh}
									/>

									{#if !selectMode}
										<IconButton
											design
											icon={pencilIcon}
											ariaLabel={t('rules.selectMode')}
											onclick={() => {
												menu = false;
												selectMode = true;
											}}
										/>
										<IconButton
											design
											icon={menuIcon}
											ariaLabel={t('common.menu')}
											expanded={menu}
											onclick={() => (menu = !menu)}
										/>
									{/if}
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
									placeholder={t('rules.searchPlaceholder') ||
										'Поиск правил...'}
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

						{#if !selectMode}
							<IconButton
								design
								icon={addIcon}
								ariaLabel={t('common.add')}
								size="lg"
								variant="surface"
								class="add-rule-btn"
								disabled={!$catalogStore.servers.length}
								onclick={add}
							/>
						{/if}
					</div>
				</div>
				{#if error}<p role="alert" transition:rise>{error}</p>{/if}
				<div class="rules-list">
					{#each filtered as rule (rule.id)}
						{@const server = serversById.get(rule.serverId)}
						{@const isDirect = rule.serverId === 'direct'}
						{@const isBlock = rule.serverId === 'block'}
						{@const ruleSubscription = subscriptionsById.get(rule.serverId)}
						{@const isSub =
							rule.serverId === 'subscription' || Boolean(ruleSubscription)}
						{@const serverName = isDirect
							? t('rules.direct')
							: isBlock
								? t('rules.block') || 'Блокировать'
								: isSub
									? (ruleSubscription?.name ??
										activeSub?.name ??
										t('rules.subscriptionServer'))
									: server
										? serverDisplayName(server)
										: '—'}
						{@const matcher =
							rule.matcher ?? (rule.isDomain ? 'domain' : 'application')}
						{@const isGeo = matcher === 'geosite' || matcher === 'geoip'}
						{@const ruleIcon = getRuleIcon(rule)}
						<article
							class="rule-row"
							class:application-rule={matcher === 'application'}
							animate:flip={{ duration: 260, easing: easeOut }}
							out:pop={{ start: 0.96, duration: 160 }}
						>
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
									onclick={() => {
										error = '';
										editorStep = 'form';
										editing = { ...rule };
									}}
									aria-label={t('rules.edit', { name: rule.name })}
								>
									<span
										class="rule-name"
										title={formatRuleDisplayName(rule.name)}
									>
										{matcher === 'final'
											? t('rules.everythingElse') || 'Всё остальное'
											: formatRuleDisplayName(rule.name)}
									</span>
									<span
										class="rule-details"
										class:domain-details={matcher === 'domain'}
									>
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
							<div class="rule-right">
								<button
									class="server-pill"
									title={serverName}
									onclick={() => {
										pickerSearchSignal = 0;
										selectingRule = rule;
										pendingServerId = rule.serverId;
									}}
									aria-label={rule.name +
										': ' +
										t('servers.title') +
										' — ' +
										serverName}
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
											<img src={tunIcon} alt="" width="24" height="24" />
										</span>
									{:else if server}
										<span class="flag-sm">
											<CountryFlag countryCode={server.countryCode} size={24} />
										</span>
									{/if}
									<span class="server-pill-name">
										{isSub && matcher !== 'application' ? 'Tun' : serverName}
									</span>
								</button>
								{#if selectMode}
									<span class="row-control" in:pop>
										<RoundCheckbox
											checked={selectedNames.has(rule.id)}
											ariaLabel={t('rules.select', { name: rule.name })}
											onclick={() =>
												(selected = selectedNames.has(rule.id)
													? selected.filter((id) => id !== rule.id)
													: [...selected, rule.id])}
										/>
									</span>
								{:else}
									<span class="row-control" in:pop>
										<ToggleSwitch
											checked={rule.enabled}
											mode="switch"
											appearance={matcher === 'application' ? 'pill' : 'dot'}
											ariaLabel={t('rules.toggle', { name: rule.name })}
											onchange={() => toggle(rule)}
										/>
									</span>
								{/if}
							</div>
						</article>
					{:else}
						<div class="empty-rules-container">
							<p class="rules-empty">{t('rules.empty')}</p>
						</div>
					{/each}
				</div>
				{#if selectMode}
					<div class="design-action-bar select-bar" transition:rise>
						<button
							type="button"
							class="delete-selected"
							aria-label={t('rules.deleteSelected')}
							disabled={!selected.length}
							onclick={() => (deleting = true)}
						>
							<img src={trashIcon} alt="" width="34" height="34" />
						</button>
						<IconButton
							design
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
				{#if menu}
					<PopupMenu top="70px" right="58px">
						<button
							class="rules-menu-item"
							type="button"
							onclick={() => {
								menu = false;
								showPresets = true;
							}}
						>
							<MaterialIcon name="tune" size={18} />{t('rules.presets')}
						</button>
						<MenuItem
							icon={checkIcon}
							label={t('rules.selectAll')}
							onclick={() => {
								menu = false;
								selectMode = true;
								selected = filtered.map((rule) => rule.id);
							}}
						/>
					</PopupMenu>
				{/if}
			</section>
		{/if}
	</div>
{/key}

{#if deleting}
	<ConfirmDialog
		title={t('common.areYouSure')}
		description={t('rules.deleteConfirm', { count: selected.length })}
		cancelLabel={t('common.cancel')}
		confirmLabel={t('common.continue')}
		onCancel={() => (deleting = false)}
		onConfirm={remove}
		destructive
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
	.rules-screen {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		min-height: 0;
	}
	.row-control {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
	}
	.empty-rules-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		padding: 6px 16px 0;
	}

	.rules-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 12px;
		padding-top: 0;
		position: relative;
		min-height: 0;
		height: 100%;
	}
	.server-picker-view {
		position: relative;
		height: 100%;
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 10px;
		overflow: hidden;
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
		padding: 0;
	}
	.header-pill {
		position: relative;
		height: var(--design-header-height);
		background: var(--design-panel);
		border-radius: 124px;
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		box-sizing: border-box;
		flex: 1;
		min-width: 0;
		overflow: hidden;
		border: 0;
		transition: border-color 180ms ease;
	}

	.header-pill:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
	}

	.header-normal {
		width: 100%;
		height: 100%;
		padding: 0 20px 0 23px;
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
		font-family: 'Satoshi', var(--font-family-base);
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		font-size: 16px;
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
		gap: 6px;
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
		width: 32px;
		height: 32px;
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
		margin: 0 auto;
		color: rgba(255, 255, 255, 0.25);
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 400;
		line-height: 1;
		letter-spacing: -0.28px;
		text-align: center;
	}
	.rule-row {
		min-height: var(--design-row-height);
		height: var(--design-row-height);
		flex-shrink: 0;
		border-radius: 124px;
		padding: 10px 12px;
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
		gap: 6px;
		min-width: 0;
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
	.rule-right {
		display: flex;
		align-items: center;
		gap: 16px;
		flex-shrink: 0;
	}
	.server-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 32px;
		min-height: 0;
		background: var(--design-panel);
		border: none;
		outline: none;
		box-shadow: none;
		border-radius: 124px;
		padding: 4px 12px 4px 4px;
	}
	.server-pill:hover {
		background: #00000073;
	}
	.flag-sm {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.flag-sm img {
		display: block;
		filter: brightness(0) invert(1);
		opacity: 0.75;
	}
	.server-pill-name {
		font-size: 12px;
		line-height: 15.6px;
		font-weight: 400;
		color: #fff;
		white-space: nowrap;
		max-width: 80px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.select-all-row {
		display: flex;
		justify-content: flex-end;
		flex-shrink: 0;
		height: 28px;
		margin-bottom: 16px;
		padding-right: 16px;
	}
	.select-all-btn {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 0;
		border: 0;
		background: transparent;
		color: #fff;
		font-family: 'Satoshi', var(--font-family-base);
		font-size: 12px;
		font-weight: 500;
		line-height: 16.2px;
	}
	/* Red delete sits where the nav pill was. */
	.delete-selected {
		display: grid;
		place-items: center;
		width: 42px;
		height: 42px;
		margin-left: 0;
		padding: 0;
		border: 1px solid #ffffff26;
		border-radius: 50%;
		background: #d03441;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			background-color 160ms ease,
			opacity 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.delete-selected img {
		filter: brightness(0) invert(1);
	}
	.delete-selected:disabled {
		opacity: 0.5;
	}
	.delete-selected:active:not(:disabled) {
		transform: scale(0.94);
	}
	@media (hover: hover) and (pointer: fine) {
		.delete-selected:hover:not(:disabled) {
			background: #e03d4b;
		}
	}

	.domain-details {
		display: none;
	}
	.rule-left {
		flex: 1;
	}
	.rules-menu-item {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		min-height: 40px;
		border: 0;
		border-radius: 14px;
		background: #ffffff0a;
		padding: 0 12px;
		font-size: 13px;
	}
	.rules-menu-item:hover {
		background: #ffffff20;
	}
	.selection .header-pill {
		width: 100%;
	}
	.empty .title-text {
		font-family: 'Satoshi', var(--font-family-base);
	}
	@media (max-width: 350px) {
		.header-normal {
			padding-left: 16px;
			padding-right: 12px;
		}
		.header-card-actions {
			gap: 2px;
		}
		.rule-right {
			gap: 6px;
		}
		.server-pill-name {
			max-width: 58px;
		}
	}
</style>
