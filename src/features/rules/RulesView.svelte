<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { runtime } from '../../lib/runtime';
	import { getProcesses, getRuleIcons } from '../../lib/services/processes';
	import { catalog } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { t } from '../../lib/i18n';
	import { serverDisplayName } from '../../lib/utils/serverDisplayName';
	import type { Rule, Server } from '../../lib/services/catalog';
	import ConfirmDialog from '../../components/ConfirmDialog.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import RoundCheckbox from '../../components/RoundCheckbox.svelte';
	import RuleEditorView from './RuleEditorView.svelte';
	import ProcessPickerView from './ProcessPickerView.svelte';
	import PresetsView from './PresetsView.svelte';
	import RuleIdentity from './parts/RuleIdentity.svelte';
	import RuleGroupIdentity from './parts/RuleGroupIdentity.svelte';
	import RuleServerControls from './parts/RuleServerControls.svelte';
	import ServerPickerPanel from './parts/ServerPickerPanel.svelte';
	import {
		groupEnabled,
		groupRules,
		type RuleGroup,
		type RuleListItem,
	} from './ruleGroups';
	import {
		findRuleIcon,
		registerRuleIcons,
		ruleIconFor,
		ruleMatcherOf,
		formatRuleDisplayName,
		persistentRuleIconCache,
	} from './ruleDisplay';
	import { flip } from 'svelte/animate';
	import {
		screen,
		screenOut,
		rise,
		pop,
		reveal,
		easeOut,
	} from '../../lib/motion';
	import addIcon from '../../assets/icons/plus.svg';
	import pencilIcon from '../../assets/icons/edit.svg';
	import { tabHeader } from '../../app/tabHeader.svelte';
	import type { SelectionBar } from '../../app/shell.svelte';

	interface Props {
		onChromeChange?: (mode: 'default' | 'selection' | 'picker') => void;
		/** Selection mode hands delete and done to the bottom bar. */
		onSelectionChange?: (bar: SelectionBar | null) => void;
		/** The list screen gives the bottom bar its presets button. */
		onPresetsChange?: (open: (() => void) | null) => void;
	}

	let { onChromeChange, onSelectionChange, onPresetsChange }: Props = $props();

	const catalogStore = catalog.state;

	// Local state
	let selectMode = $state(false);
	let selectingRule = $state<Rule | null>(null);
	let pendingServerId = $state('');
	let editing = $state<Rule | null>(null);
	let editorStep = $state<'type' | 'form'>('form');
	let choosingProcess = $state(false);
	let choosingServer = $state(false);
	let deleting = $state(false);
	let selected = $state<string[]>([]);
	let error = $state('');
	let showPresets = $state(false);
	/** Keys of the rule groups the user opened. */
	let expanded = $state<string[]>([]);
	/** Set while the server picker edits a whole group. */
	let selectingGroup = $state<RuleGroup | null>(null);
	let confirmSwitchToRules = $state(false);
	let switchReason = $state<'preset' | 'catchAll'>('preset');

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
	const normalizedQuery = $derived(tabHeader.query.toLowerCase());
	let filtered = $derived(
		normalizedQuery
			? $catalogStore.rules.filter((rule) =>
					rule.name.toLowerCase().includes(normalizedQuery),
				)
			: $catalogStore.rules,
	);
	// A search shows every matching rule on its own row.
	const listItems: RuleListItem[] = $derived(
		normalizedQuery
			? filtered.map((rule) => ({ type: 'rule', rule }))
			: groupRules($catalogStore.rules),
	);
	const expandedKeys = $derived(new Set(expanded));

	const appliedPresetIds = $derived(
		new Set(
			$catalogStore.rules
				.map((r) => r.presetId)
				.filter((id): id is string => Boolean(id)),
		),
	);
	// One catch-all per list: it decides where everything unmatched goes.
	const manualCatchAllId = $derived(
		$catalogStore.rules.find((r) => r.matcher === 'final' && !r.presetId)?.id,
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
		if (registerRuleIcons(newIcons)) {
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
			(r) => ruleMatcherOf(r) === 'application',
		);
		// Local deduplication scratch space; it is never exposed as reactive state.
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		const needed = new Set<string>();
		for (const r of appRules) {
			if (!findRuleIcon(r.name, processIcons)) {
				needed.add(r.name);
				needed.add(formatRuleDisplayName(r.name));
			}
		}
		if (needed.size > 0) {
			void getRuleIcons(Array.from(needed)).then((icons) => {
				registerIcons(icons);
			});
		}
	});

	function getRuleIcon(rule: Rule): string | null {
		if (ruleMatcherOf(rule) !== 'application') return null;
		return ruleIconFor(rule.name, processIcons);
	}

	/** What the rule row shows as its server; geo and catch-all rows get their own label. */
	function serverLabel(
		rule: Rule,
		server: Server | undefined,
		ruleSubscriptionName: string | undefined,
	): string {
		if (rule.serverId === 'direct') return t('rules.direct');
		if (rule.serverId === 'block') return t('rules.block') || 'Блокировать';
		if (isSubscriptionRule(rule))
			return (
				ruleSubscriptionName ?? activeSub?.name ?? t('rules.subscriptionServer')
			);
		return server ? serverDisplayName(server) : '—';
	}

	function isSubscriptionRule(rule: Rule): boolean {
		return (
			rule.serverId === 'subscription' || subscriptionsById.has(rule.serverId)
		);
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

	// The list screen fills the shared header; App hides it on the other screens.
	const headerOwner = {};
	$effect(() => {
		tabHeader.claim(headerOwner, {
			key: 'rules',
			title: activeSub?.name ?? t('rules.title'),
			searchPlaceholder: t('rules.searchPlaceholder') || 'Поиск правил...',
			actions: headerActions,
			hidden: selectMode,
		});
	});
	onDestroy(() => tabHeader.release(headerOwner));

	$effect(() => {
		onPresetsChange?.(
			rulesScreen === 'list' && !selectMode ? () => (showPresets = true) : null,
		);
	});
	onDestroy(() => onPresetsChange?.(null));

	$effect(() => {
		onSelectionChange?.(
			selectMode
				? {
						count: selected.length,
						onDelete: () => (deleting = true),
						onDone: () => {
							selectMode = false;
							selected = [];
						},
					}
				: null,
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
	// Read by both the entering and the leaving screen, so it is worked out
	// once per change, before the DOM swaps them.
	let rulesDir = $state(1);
	let shownScreen = 'list';
	$effect.pre(() => {
		const next = rulesScreen;
		if (next === shownScreen) return;
		rulesDir =
			(screenDepth[next] ?? 0) >= (screenDepth[shownScreen] ?? 0) ? 1 : -1;
		shownScreen = next;
	});

	// Action handlers
	async function pickServer(server: { id: string }) {
		if (!selectingRule) return;
		pendingServerId = server.id;
		try {
			if (selectingGroup) {
				await catalog.saveRules(
					selectingGroup.rules.map((rule) => ({
						...rule,
						serverId: server.id,
					})),
				);
			} else {
				await catalog.saveRule({ ...selectingRule, serverId: server.id });
			}
			selectingRule = null;
			selectingGroup = null;
			pendingServerId = '';
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function groupLabel(group: RuleGroup): string {
		return t(
			group.kind === 'addresses' ? 'rules.groupAddresses' : 'rules.groupSites',
			{ count: group.rules.length },
		);
	}

	/** One rule that stands for the whole group in the shared row controls. */
	function groupRow(group: RuleGroup): Rule {
		return {
			...group.rules[0],
			name: groupLabel(group),
			enabled: groupEnabled(group),
		};
	}

	function toggleExpanded(key: string) {
		expanded = expandedKeys.has(key)
			? expanded.filter((k) => k !== key)
			: [...expanded, key];
	}

	async function toggleGroup(group: RuleGroup) {
		const enabled = !groupEnabled(group);
		try {
			await catalog.saveRules(
				group.rules.map((rule) => ({ ...rule, enabled })),
			);
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function groupSelected(group: RuleGroup): boolean {
		return group.rules.every((rule) => selectedNames.has(rule.id));
	}

	function toggleGroupSelected(group: RuleGroup) {
		const ids = new Set(group.rules.map((rule) => rule.id));
		const rest = selected.filter((id) => !ids.has(id));
		selected = groupSelected(group) ? rest : [...rest, ...ids];
	}

	function openGroupServerPicker(group: RuleGroup) {
		selectingGroup = group;
		selectingRule = groupRow(group);
		pendingServerId = group.serverId;
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
			// New rules follow the current subscription server, not a fixed one.
			serverId: 'subscription',
			enabled: true,
		};
	}

	function openRule(rule: Rule) {
		error = '';
		editorStep = 'form';
		editing = { ...rule };
	}

	function openServerPicker(rule: Rule) {
		selectingGroup = null;
		selectingRule = rule;
		pendingServerId = rule.serverId;
	}

	async function handleApplyPreset(presetId: string) {
		try {
			await catalog.applyPreset(presetId);
			if ($catalogStore.routingMode === 'global') {
				showPresets = false;
				switchReason = 'preset';
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

	function toggleSelected(ruleId: string) {
		selected = selectedNames.has(ruleId)
			? selected.filter((id) => id !== ruleId)
			: [...selected, ruleId];
	}

	async function saveEditedRule(updated: Rule | Rule[]) {
		try {
			if (Array.isArray(updated)) {
				await catalog.saveRules(updated);
			} else {
				await catalog.saveRule(updated);
			}
			// The catch-all only applies in Rules mode.
			if (
				!Array.isArray(updated) &&
				updated.matcher === 'final' &&
				$catalogStore.routingMode === 'global'
			) {
				switchReason = 'catchAll';
				confirmSwitchToRules = true;
			}
			editing = null;
			choosingProcess = false;
			choosingServer = false;
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function closeEditor() {
		editing = null;
		choosingProcess = false;
		choosingServer = false;
	}
</script>

{#snippet headerActions()}
	<IconButton
		design
		icon={pencilIcon}
		ariaLabel={t('rules.selectMode')}
		onclick={() => (selectMode = true)}
	/>
	<IconButton
		design
		icon={addIcon}
		ariaLabel={t('common.add')}
		disabled={!$catalogStore.servers.length}
		onclick={add}
	/>
{/snippet}

{#snippet ruleRow(rule: Rule, nested: boolean)}
	{@const server = serversById.get(rule.serverId)}
	{@const ruleSubscription = subscriptionsById.get(rule.serverId)}
	{@const matcher = ruleMatcherOf(rule)}
	<article
		class="rule-row"
		class:nested
		class:application-rule={matcher === 'application'}
	>
		<RuleIdentity
			{rule}
			{matcher}
			{nested}
			serverName={serverLabel(rule, server, ruleSubscription?.name)}
			ruleIcon={getRuleIcon(rule)}
			onEdit={() => openRule(rule)}
		/>
		<RuleServerControls
			{rule}
			{matcher}
			{server}
			compact={nested}
			serverName={serverLabel(rule, server, ruleSubscription?.name)}
			isSub={isSubscriptionRule(rule)}
			{selectMode}
			checked={selectedNames.has(rule.id)}
			onPickServer={() => openServerPicker(rule)}
			onToggle={() => toggle(rule)}
			onSelect={() => toggleSelected(rule.id)}
		/>
	</article>
{/snippet}

{#key rulesScreen}
	<div
		class="rules-screen"
		in:screen={{ dir: rulesDir }}
		out:screenOut={{ dir: rulesDir, pin: true }}
	>
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
				<ServerPickerPanel
					selectedId={editing.serverId}
					onSelect={(server) => {
						if (editing) editing.serverId = server.id;
						choosingServer = false;
					}}
					onConfirm={() => {
						choosingServer = false;
					}}
					onBack={() => {
						choosingServer = false;
					}}
				/>
			{:else}
				{@const currentServer = serversById.get(editing.serverId)}
				<RuleEditorView
					rule={editing}
					server={currentServer}
					subscriptionName={activeSub?.name}
					step={editorStep}
					isNew={!$catalogStore.rules.some((rule) => rule.id === editing?.id)}
					allowFinal={!manualCatchAllId || manualCatchAllId === editing.id}
					onStepChange={(next) => (editorStep = next)}
					onNameChange={(name) => {
						if (editing) editing.name = name;
					}}
					onSave={saveEditedRule}
					onCancel={closeEditor}
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
			<ServerPickerPanel
				selectedId={pendingServerId}
				onSelect={pickServer}
				onConfirm={confirmServer}
			/>
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
							<span>{t('rules.selectAll')}</span>
							<RoundCheckbox
								asSpan
								checked={filtered.length > 0 &&
									selected.length === filtered.length}
							/>
						</button>
					</div>
				{/if}
				{#if error}<p role="alert" transition:rise>{error}</p>{/if}
				<div class="rules-list">
					{#each listItems as item (item.type === 'rule' ? item.rule.id : item.group.key)}
						<div
							class="rule-item"
							animate:flip={{ duration: 260, easing: easeOut }}
							out:pop={{ start: 0.96, duration: 160 }}
						>
							{#if item.type === 'rule'}
								{@render ruleRow(item.rule, false)}
							{:else}
								{@const group = item.group}
								{@const open = expandedKeys.has(group.key)}
								{@const row = groupRow(group)}
								{@const groupServer = serversById.get(group.serverId)}
								{@const groupServerName = serverLabel(
									row,
									groupServer,
									subscriptionsById.get(group.serverId)?.name,
								)}
								<div class="rule-group" class:open>
									<article class="rule-row group-head">
										<RuleGroupIdentity
											{group}
											label={row.name}
											subscriptionName={subscriptionsById.get(
												group.subscriptionId,
											)?.name}
											{open}
											onToggleOpen={() => toggleExpanded(group.key)}
										/>
										<RuleServerControls
											rule={row}
											matcher="domain"
											server={groupServer}
											serverName={groupServerName}
											isSub={isSubscriptionRule(row)}
											{selectMode}
											checked={groupSelected(group)}
											onPickServer={() => openGroupServerPicker(group)}
											onToggle={() => toggleGroup(group)}
											onSelect={() => toggleGroupSelected(group)}
										/>
									</article>
									{#if open}
										<div
											class="group-rules"
											transition:reveal={{ duration: 280 }}
										>
											{#each group.rules as rule (rule.id)}
												{@render ruleRow(rule, true)}
											{/each}
										</div>
									{/if}
								</div>
							{/if}
						</div>
					{:else}
						<div class="empty-rules-container">
							<p class="rules-empty">{t('rules.empty')}</p>
						</div>
					{/each}
				</div>
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
		description={switchReason === 'catchAll'
			? t('rules.switchModeDesc')
			: t('presets.switchModeDesc') ||
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
	.empty-rules-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		padding: 0 16px;
	}

	/* The list sits under the shared header, which floats over the top. In
	   selection mode "Select all" takes the header's place, so the list does
	   not move. */
	.rules-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 12px;
		padding-top: var(--tab-header-space);
		position: relative;
		min-height: 0;
		height: 100%;
	}
	.rules-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
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
	/* Figma Component 23: a 48px row, 8px above and below the 32px content. */
	.rule-row {
		min-height: 48px;
		height: 48px;
		flex-shrink: 0;
		border-radius: 32px;
		padding: 8px 12px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		gap: 8px;
	}
	.rule-row:hover {
		background: #ffffff0a;
	}
	.rule-item {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex-shrink: 0;
	}
	/* A group is a 24px card: 12px padding, 16px between its rows. It is only
	   filled while open. */
	.rule-group {
		display: flex;
		flex-direction: column;
		gap: 0;
		padding: 12px;
		border-radius: 24px;
		background: transparent;
		transition: background-color 200ms ease;
	}
	.rule-group.open {
		background: var(--design-panel);
	}
	.rule-group .rule-row {
		height: 32px;
		min-height: 32px;
		padding: 0;
		border-radius: 0;
		background: none;
	}
	/* Its own top margin, so the height reveal also opens the 16px gap. */
	.group-rules {
		display: flex;
		flex-direction: column;
		gap: 16px;
		flex-shrink: 0;
		margin-top: 16px;
	}
	/* Its rules: 24px rows without the icon, 4px in from the head. */
	.rule-group .rule-row.nested {
		height: 24px;
		min-height: 24px;
		padding-left: 4px;
	}
	.select-all-row {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 1;
		display: flex;
		justify-content: flex-end;
		align-items: center;
		height: var(--design-tab-header-height);
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
</style>
