<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { runtime } from '../../runtime';
	import { getProcesses } from '../../services/processes';
	import { createDeferredTask } from '../../utils/deferredTask';
	import { catalog } from '../../services';
	import { errorCode } from '../../services/errors';
	import { t } from '../../i18n';
	import type { Rule } from '../../services/catalog';
	import Dialog from '../../components/Dialog.svelte';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import PopupMenu from '../../components/PopupMenu.svelte';
	import MenuItem from '../../components/MenuItem.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import RoundCheckbox from '../../components/RoundCheckbox.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import ServersView from '../servers/ServersView.svelte';
	import RuleEditorView from './RuleEditorView.svelte';
	import ProcessPickerView from './ProcessPickerView.svelte';
	import searchIcon from '../../../assets/search.svg';
	import reloadIcon from '../../../assets/reload.svg';
	import menuIcon from '../../../assets/menu.svg';
	import addIcon from '../../../assets/add.svg';
	import pencilIcon from '../../../assets/pencil.svg';
	import trashIcon from '../../../assets/trash.svg';
	import shareIcon from '../../../assets/share.svg';
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

	let menu = $state(false);
	let selectMode = $state(false);
	let selectingRule = $state<Rule | null>(null);
	let pendingServerId = $state('');
	let editing = $state<Rule | null>(null);
	let choosingProcess = $state(false);
	let choosingServer = $state(false);
	let deleting = $state(false);
	let selected = $state<string[]>([]);
	let error = $state('');
	let exported = $state('');

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

	let processIcons = $state<Record<string, string>>({});
	onMount(() => {
		if (runtime.isTauri) {
			void getProcesses().then((items) => {
				const map: Record<string, string> = {};
				for (const item of items) {
					if (item.icon) {
						map[item.name.toLowerCase()] = item.icon;
					}
				}
				processIcons = map;
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
		if (rule.isDomain) return null;
		const baseName = formatRuleDisplayName(rule.name).toLowerCase();
		if (baseName.includes('chrome')) return chromeIcon;
		if (baseName.includes('discord')) return discordIcon;
		if (baseName.includes('yandex')) return yandexIcon;
		if (baseName.includes('telegram')) return telegramIcon;
		if (baseName.includes('steam')) return steamIcon;
		if (processIcons[baseName]) return processIcons[baseName];
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
			isDomain: true,
			serverId: $catalogStore.selectedServerId || 'subscription',
			enabled: true,
		};
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
	onclick={(e) => {
		if (menu) {
			const target = e.target as HTMLElement | null;
			if (!target?.closest('.popup-menu') && !target?.closest('.icon-btn')) {
				menu = false;
			}
		}
	}}
/>

{#if editing}
	{#if choosingProcess}
		<ProcessPickerView
			selectedProcess={editing.name}
			onSelect={(procs: string[]) => {
				if (editing) {
					editing.name = procs.join(', ');
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
									icon={menuIcon}
									ariaLabel={t('common.menu')}
									active={menu}
									expanded={menu}
									onclick={() => (menu = !menu)}
								/>
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
				{#if menu && !searching}
					<PopupMenu top="68px" right="58px">
						<MenuItem
							icon={pencilIcon}
							label={t('common.edit')}
							onclick={() => {
								selectMode = true;
								menu = false;
							}}
						/>
						<MenuItem
							icon={reloadIcon}
							label={t('common.reload')}
							onclick={() => {
								menu = false;
								void refresh();
							}}
						/>
						<MenuItem
							icon={shareIcon}
							label={t('common.export')}
							disabled={!$catalogStore.rules.length}
							onclick={() => {
								exported = JSON.stringify(
									{ version: 1, demo: true, rules: $catalogStore.rules },
									null,
									2,
								);
								menu = false;
							}}
						/>
						<MenuItem
							icon={trashIcon}
							label={t('common.delete')}
							variant="danger"
							onclick={() => {
								selectMode = true;
								menu = false;
							}}
						/>
					</PopupMenu>
				{/if}
			</div>
		{/if}
		{#if error}<p role="alert">{error}</p>{/if}
		<div class="rules-list">
			{#each filtered as rule (rule.id)}
				{@const server = serversById.get(rule.serverId)}
				{@const isDirect = rule.serverId === 'direct'}
				{@const isSub =
					rule.serverId === 'subscription' ||
					(activeSub && rule.serverId === activeSub.id)}
				{@const serverName = isDirect
					? t('rules.direct')
					: isSub
						? (activeSub?.name ?? t('rules.subscriptionServer'))
						: server
							? t('servers.countries.' + server.countryCode)
							: '—'}
				{@const ruleIcon = getRuleIcon(rule)}
				<article class="rule-row">
					<div class="rule-left">
						<div
							class="app-icon-wrapper"
							class:domain-icon={!ruleIcon}
						>
							{#if ruleIcon}
								<img
									class="app-icon"
									src={ruleIcon}
									alt=""
								/>
							{:else}
								<span class="globe-text" aria-hidden="true">
									<MaterialIcon
										name={rule.isDomain ? 'public' : 'description'}
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
							<span class="rule-details">{serverName}</span>
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
			{:else}<p>{t('rules.empty')}</p>{/each}
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
	<Dialog title={t('common.delete')} onClose={() => (deleting = false)}>
		<p>{t('rules.deleteConfirm', { count: selected.length })}</p>
		{#if error}
			<p role="alert">{error}</p>
		{/if}
		<button class="danger" onclick={remove}>{t('common.delete')}</button>
	</Dialog>
{/if}

{#if exported}
	<Dialog title={t('common.export')} onClose={() => (exported = '')}>
		<p>{t('demo.export')}</p>
		<textarea
			readonly
			value={exported}
			rows="10"
			aria-label={t('common.export')}></textarea>
	</Dialog>
{/if}

<style>
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
		font-size: 11px;
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
