<script lang="ts">
	import type { Rule, RuleMatcher, Server } from '../../services/catalog';
	import { t } from '../../i18n';
	import { serverDisplayName } from '../../utils/serverDisplayName';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import chevronIcon from '../../../assets/chevron-down.svg';
	import checkIcon from '../../../assets/check.svg';

	interface Props {
		rule: Rule;
		server?: Server;
		subscriptionName?: string;
		onSave: (rule: Rule | Rule[]) => void;
		onCancel: () => void;
		onSelectProcess: () => void;
		onSelectServer: () => void;
		onMatcherChange: (matcher: RuleMatcher) => void;
	}

	let {
		rule,
		server,
		subscriptionName,
		onSave,
		onCancel,
		onSelectProcess,
		onSelectServer,
		onMatcherChange,
	}: Props = $props();

	// svelte-ignore state_referenced_locally
	let matcher = $state<RuleMatcher>(
		rule.matcher ?? (rule.isDomain ? 'domain' : 'application'),
	);

	function initialName(r: Rule, m: RuleMatcher): string {
		if (m === 'domain') return r.name.replace(/^https?:\/\//i, '');
		if (m === 'geosite') return r.name.replace(/^geosite:/i, '');
		if (m === 'geoip') return r.name.replace(/^geoip:/i, '');
		return r.name;
	}

	// svelte-ignore state_referenced_locally
	let name = $state(initialName(rule, matcher));
	let serverId = $derived(rule.serverId);
	let error = $state('');

	const matcherIndex = $derived(
		['application', 'domain', 'geosite', 'geoip'].indexOf(matcher),
	);

	function cleanDomain(val: string): string {
		// Preserve IP network prefixes; the native backend validates address and prefix length.
		if (/^[\da-f:.]+\/\d{1,3}$/i.test(val.trim())) return val.trim();
		const cleaned = val
			.trim()
			.replace(/^https?:\/\//i, '')
			.replace(/[/?#].*$/, '');
		return cleaned;
	}

	function handleDomainPaste(event: ClipboardEvent) {
		const pasted = event.clipboardData?.getData('text');
		if (!pasted) return;
		event.preventDefault();
		name = cleanDomain(pasted);
		error = '';
	}

	function handleGeoPaste(event: ClipboardEvent) {
		const pasted = event.clipboardData?.getData('text');
		if (!pasted) return;
		event.preventDefault();
		const trimmed = pasted.trim();
		const lower = trimmed.toLowerCase();
		const wrongPrefix = matcher === 'geosite' ? 'geoip:' : 'geosite:';
		const expectedPrefix = matcher === 'geosite' ? 'geosite:' : 'geoip:';

		if (lower.startsWith(wrongPrefix)) {
			error =
				t('rules.mismatchedPrefixError', {
					expected: expectedPrefix,
					wrong: wrongPrefix,
				}) || `Неверный префикс: для ${matcher} используйте ${expectedPrefix}`;
			return;
		}

		if (lower.startsWith(expectedPrefix)) {
			name = trimmed.slice(expectedPrefix.length).trim();
		} else {
			name = trimmed;
		}
		error = '';
	}

	function switchMatcher(next: RuleMatcher) {
		matcher = next;
		onMatcherChange(next);
		error = '';
	}

	function handleSave() {
		error = '';
		const targetServer = serverId || 'subscription';

		if (matcher === 'domain') {
			const finalName = cleanDomain(name);
			if (!finalName) {
				error = t('errors.invalidRule');
				return;
			}
			onSave({
				...rule,
				matcher: 'domain',
				isDomain: true,
				name: finalName,
				serverId: targetServer,
			});
		} else if (matcher === 'application') {
			const procs = name
				.split(',')
				.map((s) => s.trim())
				.filter(Boolean);
			if (procs.length === 0) {
				error = t('rules.chooseProcessError') || 'Выберите процесс';
				return;
			}
			if (procs.length === 1) {
				onSave({
					...rule,
					matcher: 'application',
					isDomain: false,
					name: procs[0],
					serverId: targetServer,
				});
			} else {
				const rules: Rule[] = [
					{
						...rule,
						matcher: 'application',
						isDomain: false,
						name: procs[0],
						serverId: targetServer,
					},
					...procs.slice(1).map((p) => ({
						id: crypto.randomUUID(),
						name: p,
						matcher: 'application' as RuleMatcher,
						isDomain: false,
						serverId: targetServer,
						enabled: true,
					})),
				];
				onSave(rules);
			}
		} else {
			// GeoSite or GeoIP
			let tag = name.trim();
			const lower = tag.toLowerCase();
			const wrongPrefix = matcher === 'geosite' ? 'geoip:' : 'geosite:';
			const expectedPrefix = matcher === 'geosite' ? 'geosite:' : 'geoip:';

			if (lower.startsWith(wrongPrefix)) {
				error =
					t('rules.mismatchedPrefixError', {
						expected: expectedPrefix,
						wrong: wrongPrefix,
					}) ||
					`Неверный префикс: для ${matcher} используйте ${expectedPrefix}`;
				return;
			}

			if (lower.startsWith(expectedPrefix)) {
				tag = tag.slice(expectedPrefix.length).trim();
			}

			if (
				!/^[a-zA-Z0-9_\-!@]+$/.test(tag) ||
				tag.length === 0 ||
				tag.length > 128
			) {
				error =
					t('rules.invalidGeoTag') ||
					'Недопустимый тег. Используйте латинские буквы, цифры, _, -, !, @ (до 128 символов)';
				return;
			}

			onSave({
				...rule,
				matcher,
				isDomain: false,
				name: tag,
				serverId: targetServer,
			});
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSave();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			onCancel();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section class="rule-editor-view" aria-label={t('rules.editTitle')}>
	<div class="form-container">
		<!-- Section 1: Rule Type (Application / Domain / GeoSite / GeoIP) -->
		<div class="field-group">
			<span class="field-label">{t('rules.kind')}</span>
			<div
				class="type-segmented-pill"
				role="tablist"
				aria-label={t('rules.kind')}
			>
				<div
					class="slider-thumb"
					style:width="calc((100% - 12px - 18px) / 4)"
					style:transform="translateX(calc({matcherIndex} * (100% + 6px)))"
				></div>
				<button
					type="button"
					role="tab"
					class="segment-btn"
					class:active-segment={matcher === 'application'}
					aria-selected={matcher === 'application'}
					onclick={() => switchMatcher('application')}
				>
					{t('rules.applicationTab') || 'Программа'}
				</button>
				<button
					type="button"
					role="tab"
					class="segment-btn"
					class:active-segment={matcher === 'domain'}
					aria-selected={matcher === 'domain'}
					onclick={() => switchMatcher('domain')}
				>
					{t('rules.domainTab') || 'Домен'}
				</button>
				<button
					type="button"
					role="tab"
					class="segment-btn"
					class:active-segment={matcher === 'geosite'}
					aria-selected={matcher === 'geosite'}
					onclick={() => switchMatcher('geosite')}
				>
					GeoSite
				</button>
				<button
					type="button"
					role="tab"
					class="segment-btn"
					class:active-segment={matcher === 'geoip'}
					aria-selected={matcher === 'geoip'}
					onclick={() => switchMatcher('geoip')}
				>
					GeoIP
				</button>
			</div>
		</div>

		<!-- Section 2: Input Field based on Matcher -->
		{#if matcher === 'domain'}
			<div class="field-group">
				<label class="field-label" for="domain-input">
					{t('rules.domain')}
				</label>
				<div class="input-pill" class:active-pill={name.length > 0}>
					<span class="domain-prefix">https://</span>
					<input
						id="domain-input"
						class="text-input"
						type="text"
						bind:value={name}
						onpaste={handleDomainPaste}
						placeholder="example.com"
						autocomplete="off"
						spellcheck="false"
						required
					/>
				</div>
			</div>
		{:else if matcher === 'application'}
			<div class="field-group">
				<span class="field-label">
					{t('rules.applicationField') || 'Приложение'}
				</span>
				<button
					type="button"
					class="selector-pill"
					onclick={() => {
						onSelectProcess();
					}}
					aria-label={t('rules.selectProcess') || 'Выбрать процесс'}
				>
					<span class="selector-value" class:placeholder-val={!name}>
						{name || t('rules.selectProcess') || 'Выбрать процесс'}
					</span>
					<img src={chevronIcon} alt="" class="chevron-right" />
				</button>
			</div>
		{:else}
			<div class="field-group">
				<label class="field-label" for="geo-input">
					{matcher === 'geosite' ? 'GeoSite' : 'GeoIP'} ({t('rules.category') ||
						'Категория'})
				</label>
				<div class="input-pill" class:active-pill={name.length > 0}>
					<span class="domain-prefix">
						{matcher === 'geosite' ? 'geosite:' : 'geoip:'}
					</span>
					<input
						id="geo-input"
						class="text-input"
						type="text"
						bind:value={name}
						onpaste={handleGeoPaste}
						placeholder={matcher === 'geosite' ? 'category-ru' : 'ru'}
						autocomplete="off"
						spellcheck="false"
						required
					/>
				</div>
				<div class="geo-examples">
					<span class="example-label">{t('rules.examples') || 'Примеры'}:</span>
					<div class="example-chips">
						{#each matcher === 'geosite' ? ['category-ru', 'antifilter', 'antifilter-community'] : ['ru', 'private', 'antifilter'] as ex (ex)}
							<button
								type="button"
								class="example-tag"
								onclick={() => {
									name = ex;
									error = '';
								}}
							>
								{ex}
							</button>
						{/each}
					</div>
				</div>
			</div>
		{/if}

		<!-- Section 3: Server Selection Field -->
		<div class="field-group">
			<span class="field-label">{t('rules.server') || 'Сервер'}</span>
			<button
				type="button"
				class="selector-pill"
				onclick={onSelectServer}
				aria-label={t('rules.selectServer') || 'Выбрать сервер'}
			>
				<div class="server-info">
					{#if serverId === 'direct'}
						<MaterialIcon name="alt_route" size={20} />
						<span class="selector-value">{t('rules.direct')}</span>
					{:else if serverId === 'block'}
						<MaterialIcon name="block" size={20} />
						<span class="selector-value">
							{t('rules.block') || 'Блокировать'}
						</span>
					{:else if serverId === 'subscription' || (subscriptionName && serverId === subscriptionName)}
						<MaterialIcon name="sync_alt" size={20} />
						<span class="selector-value">
							{subscriptionName ?? t('rules.subscriptionServer')}
						</span>
					{:else if server}
						<CountryFlag countryCode={server.countryCode} size={20} />
						<span class="selector-value">
							{serverDisplayName(server)}
						</span>
					{:else}
						<span class="selector-value placeholder-val">
							{t('rules.selectServer') || 'Выбрать сервер'}
						</span>
					{/if}
				</div>
				<img src={chevronIcon} alt="" class="chevron-right" />
			</button>
		</div>

		{#if error}
			<p class="error-msg" role="alert">{error}</p>
		{/if}
	</div>

	<div class="action-bar">
		<IconButton
			icon={chevronIcon}
			ariaLabel={t('common.close')}
			size="lg"
			variant="surface"
			class="back-round"
			onclick={onCancel}
		/>
		<IconButton
			icon={checkIcon}
			ariaLabel={t('common.save')}
			size="lg"
			variant="primary"
			onclick={handleSave}
		/>
	</div>
</section>

<style>
	.rule-editor-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		flex: 1;
		min-height: 0;
		justify-content: space-between;
		box-sizing: border-box;
		padding-top: 12px;
		position: relative;
	}

	.form-container {
		display: flex;
		flex-direction: column;
		gap: 20px;
		width: 100%;
	}

	.field-group {
		display: flex;
		flex-direction: column;
		gap: 10px;
		width: 100%;
	}

	.field-label {
		font-size: 14px;
		font-weight: 500;
		color: #ffffff;
		letter-spacing: -0.2px;
	}

	/* Segmented switcher */
	.type-segmented-pill {
		position: relative;
		width: 100%;
		height: 52px;
		border-radius: 24px;
		background: rgba(0, 0, 0, 0.22);
		border: none;
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		padding: 6px;
		box-sizing: border-box;
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.slider-thumb {
		position: absolute;
		top: 6px;
		bottom: 6px;
		left: 6px;
		border-radius: 18px;
		background: #ffffff;
		box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
		transition: transform 0.28s cubic-bezier(0.22, 1, 0.36, 1);
		z-index: 1;
		pointer-events: none;
	}

	.segment-btn {
		position: relative;
		z-index: 2;
		flex: 1;
		height: 100%;
		border-radius: 18px;
		background: transparent !important;
		border: none;
		color: rgba(255, 255, 255, 0.7);
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition:
			color 200ms ease,
			font-weight 200ms ease;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0 4px;
		white-space: nowrap;
	}

	.segment-btn:hover:not(.active-segment) {
		color: #ffffff;
	}

	.segment-btn.active-segment {
		color: #000000;
		font-weight: 600;
	}

	/* Input and Selector Pills */
	.input-pill,
	.selector-pill {
		width: 100%;
		height: 56px;
		border-radius: 24px;
		background: rgba(0, 0, 0, 0.2);
		border: 1px solid transparent;
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		box-sizing: border-box;
		padding: 0 20px;
		display: flex;
		align-items: center;
		transition:
			border-color 180ms ease,
			box-shadow 180ms ease,
			background-color 180ms ease;
	}

	.selector-pill {
		cursor: pointer;
		text-align: left;
		justify-content: space-between;
		font: inherit;
		color: inherit;
	}

	.selector-pill:hover {
		background: rgba(255, 255, 255, 0.08);
	}

	.input-pill:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
		box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.12);
	}

	.server-info {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.domain-prefix {
		font-size: 14px;
		color: rgba(255, 255, 255, 0.45);
		user-select: none;
		flex-shrink: 0;
	}

	.text-input {
		width: 100%;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		border-radius: 0 !important;
		font-size: 14px;
		font-family: inherit;
		color: #ffffff;
		padding: 0 !important;
		-webkit-appearance: none;
		appearance: none;
		flex: 1;
	}

	.text-input::placeholder {
		color: rgba(255, 255, 255, 0.45);
	}

	.geo-examples {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
		padding: 0 4px;
	}

	.example-label {
		font-size: 12px;
		color: rgba(255, 255, 255, 0.5);
	}

	.example-chips {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
	}

	.example-tag {
		background: rgba(255, 255, 255, 0.08);
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 12px;
		padding: 2px 8px;
		font-size: 11px;
		color: rgba(255, 255, 255, 0.85);
		cursor: pointer;
		transition:
			background 150ms ease,
			color 150ms ease;
	}

	.example-tag:hover {
		background: rgba(255, 255, 255, 0.18);
		color: #ffffff;
	}

	.selector-value {
		font-size: 14px;
		color: #ffffff;
	}

	.selector-value.placeholder-val {
		color: rgba(255, 255, 255, 0.45);
	}

	.chevron-right {
		width: 18px;
		height: 18px;
		transform: rotate(-90deg);
		filter: brightness(0) invert(1);
		opacity: 0.6;
	}

	.error-msg {
		margin: 0;
		padding: 8px 12px;
		background: rgba(212, 42, 42, 0.2);
		border: none;
		border-radius: 16px;
		color: #ff8e8e;
		font-size: 13px;
		text-align: center;
	}

	.action-bar {
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
</style>
