<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { runtime } from '../../runtime';
	import { getProcesses } from '../../services/processes';
	import { catalog } from '../../services';
	import { createDeferredTask } from '../../utils/deferredTask';
	import yandexIcon from '../../../assets/yandex.png';
	import searchIcon from '../../../assets/search.svg';
	import checkIcon from '../../../assets/check.svg';
	import crossIcon from '../../../assets/close.svg';
	import chevronIcon from '../../../assets/chevron-down.svg';
	import IconButton from '../../components/IconButton.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import { t } from '../../i18n';

	const searchTask = createDeferredTask();
	onDestroy(searchTask.cancel);

	interface Props {
		selectedProcess?: string | string[];
		onSelect: (processNames: string[]) => void;
		onCancel: () => void;
	}

	let {
		selectedProcess = 'youtube.com/',
		onSelect,
		onCancel,
	}: Props = $props();

	interface ProcessItem {
		id: string;
		name: string;
		detail: string;
		icon?: string;
	}

	let processes = $state<ProcessItem[]>(
		runtime.isTauri
			? []
			: [
					{
						id: 'discord',
						name: 'Discord',
						detail: 'C:\\Users\\user\\AppData',
					},
					{
						id: 'yandex',
						name: 'Yandex',
						detail: 'C:\\Users\\user\\AppData',
						icon: yandexIcon,
					},
					{
						id: 'youtube',
						name: 'youtube.com/',
						detail: 'C:\\Users\\user\\AppData',
					},
					{
						id: 'chrome',
						name: 'chrome.exe',
						detail: 'C:\\Program Files\\Google\\Chrome\\Application',
					},
					{
						id: 'telegram',
						name: 'Telegram.exe',
						detail: 'C:\\Users\\user\\AppData\\Roaming\\Telegram Desktop',
					},
					{
						id: 'steam',
						name: 'steam.exe',
						detail: 'C:\\Program Files (x86)\\Steam',
					},
					{
						id: 'spotify',
						name: 'Spotify.exe',
						detail: 'C:\\Users\\user\\AppData\\Roaming\\Spotify',
					},
				],
	);
	onMount(() => {
		let disposed = false;
		if (runtime.isTauri)
			void getProcesses()
				.then((items) => {
					if (!disposed) processes = items;
				})
				.catch(() => catalog.persistenceError.set(true));
		return () => {
			disposed = true;
		};
	});

	function parseInitial(sel?: string | string[]): string[] {
		if (Array.isArray(sel)) return [...sel];
		if (typeof sel === 'string' && sel.trim()) {
			return sel
				.split(',')
				.map((s) => s.trim())
				.filter(Boolean);
		}
		return [];
	}

	// svelte-ignore state_referenced_locally
	let selected = $state<string[]>(parseInitial(selectedProcess));
	let searchMode = $state<boolean>(false);
	let searchQuery = $state<string>('');
	let searchInputEl = $state<HTMLInputElement | null>(null);

	const normalizedQuery = $derived(searchQuery.toLowerCase());
	const selectedNames = $derived(new Set(selected));
	function processKey(process: ProcessItem) {
		return process.name;
	}
	let filteredProcesses = $derived(
		processes.filter(
			(p) =>
				p.name.toLowerCase().includes(normalizedQuery) ||
				p.detail.toLowerCase().includes(normalizedQuery),
		),
	);

	let isAllSelected = $derived(
		filteredProcesses.length > 0 &&
			filteredProcesses.every((p) => selectedNames.has(processKey(p))),
	);

	function toggleSelect(name: string) {
		if (selectedNames.has(name)) {
			selected = selected.filter((n) => n !== name);
		} else {
			selected = [...selected, name];
		}
	}

	function toggleSelectAll() {
		if (isAllSelected) {
			const filteredNames = new Set(filteredProcesses.map(processKey));
			selected = selected.filter((n) => !filteredNames.has(n));
		} else {
			const set = new Set([...selected, ...filteredProcesses.map(processKey)]);
			selected = Array.from(set);
		}
	}

	let searchClosing = $state<boolean>(false);

	function handleOpenSearch() {
		searchMode = true;
		searchClosing = false;
		searchTask.schedule(() => searchInputEl?.focus(), 180);
	}

	function handleCloseSearch() {
		searchClosing = true;
		searchTask.schedule(() => {
			searchMode = false;
			searchClosing = false;
			searchQuery = '';
		}, 280);
	}

	function handleConfirm() {
		if (selected.length > 0) {
			onSelect(selected);
		} else {
			onCancel();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			if (searchMode) {
				handleCloseSearch();
			} else {
				onCancel();
			}
		} else if (e.key === 'Enter' && !searchMode) {
			e.preventDefault();
			handleConfirm();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section
	class="process-picker-view"
	aria-label={t('rules.selectProcess') || 'Выбор процесса'}
>
	<!-- Top Subheader: Select All -->
	<div class="top-row">
		<button
			type="button"
			class="select-all-btn"
			onclick={toggleSelectAll}
			aria-label="Select all"
		>
			<span class="select-all-text">Select all</span>
			<div class="round-indicator" class:checked={isAllSelected}></div>
		</button>
	</div>

	<!-- Processes List -->
	<div class="process-list" role="list">
		{#each filteredProcesses as item (item.id)}
			{@const isChecked = selectedNames.has(processKey(item))}
			<button
				type="button"
				class="process-row"
				class:selected-row={isChecked}
				onclick={() => toggleSelect(processKey(item))}
			>
				<div class="process-left">
					<div class="app-icon-box">
						{#if item.icon}
							<img src={item.icon} alt="" class="app-icon-img" />
						{:else}
							<span class="generic-file-icon">
								<MaterialIcon name="description" size={20} />
							</span>
						{/if}
					</div>
					<div class="process-info">
						<span class="process-name">{item.name}</span>
						<span class="process-detail">{item.detail}</span>
					</div>
				</div>

				<div class="process-right">
					<div class="round-indicator" class:checked={isChecked}></div>
				</div>
			</button>
		{:else}
			<p class="empty-msg">{t('rules.empty') || 'Процессы не найдены'}</p>
		{/each}
	</div>

	<!-- Bottom Navigation / Search Bar -->
	<div class="bottom-container">
		<div class="bottom-wrapper">
			<!-- Screen 15: Standard 3-button bottom bar -->
			<div class="standard-bar" class:bar-hidden={searchMode}>
				<IconButton
					icon={chevronIcon}
					ariaLabel={t('common.close')}
					size="lg"
					variant="surface"
					class="back-round"
					onclick={onCancel}
				/>
				<IconButton
					icon={searchIcon}
					ariaLabel={t('common.search')}
					size="lg"
					variant="surface"
					onclick={handleOpenSearch}
				/>
				<IconButton
					icon={checkIcon}
					ariaLabel={t('common.save')}
					size="lg"
					variant="primary"
					onclick={handleConfirm}
				/>
			</div>

			<!-- Screen 16: Expanded Search bar pill from center outward -->
			<div
				class="search-pill"
				class:search-open={searchMode && !searchClosing}
				class:search-closing={searchClosing}
			>
				<img src={searchIcon} alt="" class="search-pill-icon" />
				<input
					bind:this={searchInputEl}
					type="text"
					class="search-pill-input"
					placeholder={t('rules.searchPlaceholder') || 'Поиск процесса...'}
					bind:value={searchQuery}
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
	</div>
</section>

<style>
	.process-picker-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		flex: 1;
		min-height: 0;
		justify-content: space-between;
		box-sizing: border-box;
		padding-top: 4px;
		position: relative;
	}

	.top-row {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		width: 100%;
		padding: 0 4px 12px;
		box-sizing: border-box;
	}

	.select-all-btn {
		display: flex;
		align-items: center;
		gap: 12px;
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 0;
	}

	.select-all-text {
		font-size: 14px;
		color: rgba(255, 255, 255, 0.85);
		font-weight: 500;
	}

	.process-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding-bottom: 20px;
		scrollbar-width: none;
	}

	.process-list::-webkit-scrollbar {
		display: none;
	}

	.process-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 8px;
		border-radius: 16px;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		cursor: pointer;
		box-sizing: border-box;
		width: 100%;
		transition: background-color 150ms ease;
		font: inherit;
		color: inherit;
		text-align: left;
	}

	.process-row:hover,
	.process-row:focus-visible {
		background: rgba(255, 255, 255, 0.06) !important;
		outline: none !important;
	}

	.process-left {
		display: flex;
		align-items: center;
		gap: 14px;
		min-width: 0;
	}

	.app-icon-box {
		width: 36px;
		height: 36px;
		min-width: 36px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(255, 255, 255, 0.08);
		overflow: hidden;
	}

	.app-icon-img {
		width: 100%;
		height: 100%;
		object-fit: contain;
		image-rendering: auto;
		transform: translateZ(0);
	}

	.generic-file-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		color: #ffffff;
		opacity: 0.8;
	}

	.process-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.process-name {
		font-size: 14px;
		font-weight: 500;
		color: #ffffff;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.process-detail {
		font-size: 12px;
		color: rgba(255, 255, 255, 0.45);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.process-right {
		display: flex;
		align-items: center;
		justify-content: center;
		padding-left: 8px;
	}

	/* Round indicator: dark circle by default, solid white when selected (Figma spec) */
	.round-indicator {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: rgba(0, 0, 0, 0.35);
		border: 2px solid rgba(255, 255, 255, 0.2);
		box-sizing: border-box;
		flex-shrink: 0;
		transition:
			background-color 160ms ease,
			border-color 160ms ease,
			box-shadow 160ms ease;
	}

	.round-indicator.checked {
		background: #ffffff !important;
		border-color: #ffffff !important;
		box-shadow: 0 0 10px rgba(255, 255, 255, 0.35);
	}

	.empty-msg {
		color: rgba(255, 255, 255, 0.5);
		font-size: 14px;
		text-align: center;
		margin-top: 24px;
	}

	.bottom-container {
		width: 100%;
		flex-shrink: 0;
		margin-bottom: 33px;
	}

	.bottom-wrapper {
		position: relative;
		width: 100%;
		height: 50px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.standard-bar {
		width: 100%;
		height: 50px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0 4px;
		box-sizing: border-box;
		transition:
			opacity 200ms ease,
			transform 200ms ease;
		opacity: 1;
		transform: scale(1);
	}

	.standard-bar.bar-hidden {
		opacity: 0;
		transform: scale(0.92);
		pointer-events: none;
	}

	:global(.back-round .btn-icon) {
		transform: rotate(90deg) !important;
	}

	/* Search bar pill (Screen 16): animates width from center outward (50px -> 100%) and back */
	.search-pill {
		position: absolute;
		top: 0;
		left: 50%;
		transform: translateX(-50%);
		width: 50px;
		height: 50px;
		border-radius: 124px;
		background: rgba(0, 0, 0, 0.45);
		border: 1px solid transparent;
		-webkit-backdrop-filter: blur(16px);
		backdrop-filter: blur(16px);
		display: flex;
		align-items: center;
		padding: 0 8px 0 18px;
		box-sizing: border-box;
		gap: 12px;
		opacity: 0;
		pointer-events: none;
		overflow: hidden;
		white-space: nowrap;
		transition:
			width 280ms cubic-bezier(0.2, 0.9, 0.3, 1),
			opacity 180ms ease,
			border-color 180ms ease;
	}

	.search-pill:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
	}

	.search-pill.search-open {
		width: 100%;
		opacity: 1;
		pointer-events: auto;
	}

	.search-pill.search-closing {
		width: 50px;
		opacity: 0;
		pointer-events: none;
		transition:
			width 280ms cubic-bezier(0.2, 0.9, 0.3, 1),
			opacity 200ms ease 60ms;
	}

	.search-pill-icon {
		width: 18px;
		height: 18px;
		object-fit: contain;
		filter: brightness(0) invert(1);
		opacity: 0.6;
		flex-shrink: 0;
	}

	.search-pill-input {
		flex: 1;
		min-width: 0;
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
		opacity: 0;
		transition: opacity 140ms ease;
	}

	.search-pill.search-open .search-pill-input {
		opacity: 1;
		transition: opacity 180ms ease 90ms;
	}

	.search-pill.search-closing .search-pill-input {
		opacity: 0;
		transition: opacity 80ms ease;
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
		opacity: 0;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
		transition:
			transform 160ms var(--ease-spring),
			opacity 140ms ease;
	}

	.search-pill.search-open .close-search-btn {
		opacity: 1;
		transition:
			transform 160ms var(--ease-spring),
			opacity 180ms ease 90ms;
	}

	.search-pill.search-closing .close-search-btn {
		opacity: 0;
		transition: opacity 80ms ease;
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
</style>
