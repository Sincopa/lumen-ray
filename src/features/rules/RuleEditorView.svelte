<script lang="ts">
	import nextIcon from '../../assets/chevron-right.svg';
	import type { Rule, RuleMatcher, Server } from '../../lib/services/catalog';
	import { t } from '../../lib/i18n';
	import { screen, reveal } from '../../lib/motion';
	import { serverDisplayName } from '../../lib/utils/serverDisplayName';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import DesignTextField from '../../components/DesignTextField.svelte';
	import backIcon from '../../assets/back.svg';
	import chevronIcon from '../../assets/chevron-down.svg';
	import checkIcon from '../../assets/check.svg';

	interface Props {
		rule: Rule;
		server?: Server;
		subscriptionName?: string;
		onSave: (rule: Rule | Rule[]) => void;
		onCancel: () => void;
		onSelectProcess: () => void;
		onSelectServer: () => void;
		onMatcherChange: (matcher: RuleMatcher) => void;
		/** New rules start by picking a type. Edits open the form. */
		step?: 'type' | 'form';
		onStepChange?: (step: 'type' | 'form') => void;
		onNameChange?: (name: string) => void;
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
		step = 'form',
		onStepChange,
		onNameChange,
	}: Props = $props();

	const ruleTypes: { id: RuleMatcher; label: string; desc: string }[] = [
		{ id: 'domain', label: 'rules.typeDomain', desc: 'rules.typeDomainDesc' },
		{
			id: 'application',
			label: 'rules.typeApplication',
			desc: 'rules.typeApplicationDesc',
		},
		{ id: 'geosite', label: 'GeoSite', desc: 'rules.typeGeositeDesc' },
		{ id: 'geoip', label: 'GeoIP', desc: 'rules.typeGeoipDesc' },
	];
	let pickedType = $state<RuleMatcher | null>(null);

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

	$effect(() => {
		onNameChange?.(name);
	});

	let processes = $derived(
		name
			.split(',')
			.map((p) => p.trim())
			.filter(Boolean),
	);
	let canSave = $derived.by(() => {
		if (matcher === 'final') return true;
		if (matcher === 'domain') return cleanDomain(name).length > 0;
		if (matcher === 'application') return processes.length > 0;
		return name.trim().length > 0;
	});

	function continueWithType() {
		if (!pickedType) return;
		if (pickedType !== matcher) {
			switchMatcher(pickedType);
			name = '';
		}
		onStepChange?.('form');
	}

	function goBack() {
		if (step === 'form' && onStepChange) {
			pickedType = matcher;
			onStepChange('type');
		} else {
			onCancel();
		}
	}

	const capitalize = (value: string) =>
		value.charAt(0).toUpperCase() + value.slice(1);

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

		if (matcher === 'final') {
			onSave({
				...rule,
				matcher: 'final',
				isDomain: false,
				name: 'match',
				serverId: targetServer,
			});
			return;
		}

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
			if (step === 'type') continueWithType();
			else handleSave();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			goBack();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#snippet serverField()}
	<div class="field-group">
		<span class="field-label">{t('rules.server')}</span>
		<button
			type="button"
			class="selector-pill"
			onclick={onSelectServer}
			aria-label={t('rules.selectServer')}
		>
			<span class="server-info">
				{#if serverId === 'direct'}
					<span class="server-badge">
						<MaterialIcon name="alt_route" size={20} />
					</span>
					<span class="selector-value">{t('rules.direct')}</span>
				{:else if serverId === 'block'}
					<span class="server-badge">
						<MaterialIcon name="block" size={20} />
					</span>
					<span class="selector-value">{t('rules.block')}</span>
				{:else if serverId === 'subscription' || (subscriptionName && serverId === subscriptionName)}
					<span class="server-badge">
						<MaterialIcon name="sync_alt" size={20} />
					</span>
					<span class="selector-value">
						{subscriptionName ?? t('rules.subscriptionServer')}
					</span>
				{:else if server}
					<CountryFlag countryCode={server.countryCode} size={32} />
					<span class="selector-value">{serverDisplayName(server)}</span>
				{:else}
					<span class="selector-value placeholder-val">
						{t('rules.selectServer')}
					</span>
				{/if}
			</span>
			<img src={chevronIcon} alt="" class="chevron-right" />
		</button>
	</div>
{/snippet}

<section class="rule-editor-view" aria-label={t('rules.editTitle')}>
	{#key step}
		<div class="editor-step" in:screen={{ dir: step === 'form' ? 1 : -1 }}>
			{#if step === 'type'}
				<!-- Choose what the rule matches. -->
				<div class="type-list" role="radiogroup" aria-label={t('rules.kind')}>
					{#each ruleTypes as type (type.id)}
						{@const selected = pickedType === type.id}
						<button
							type="button"
							role="radio"
							class="type-row"
							class:selected
							aria-checked={selected}
							onclick={() => (pickedType = type.id)}
							ondblclick={() => {
								pickedType = type.id;
								continueWithType();
							}}
						>
							<span class="type-head">
								<span class="type-label">
									{type.label.startsWith('rules.') ? t(type.label) : type.label}
								</span>
								<span class="type-radio" aria-hidden="true">
									{#if selected}<span class="type-dot"></span>{/if}
								</span>
							</span>
							{#if selected}
								<span class="type-desc" transition:reveal>{t(type.desc)}</span>
							{/if}
						</button>
					{/each}
				</div>

				<div class="design-action-bar">
					<IconButton
						design
						icon={backIcon}
						ariaLabel={t('common.back')}
						size="lg"
						variant="surface"
						onclick={onCancel}
					/>
					<button
						type="button"
						class="continue-btn"
						disabled={!pickedType}
						onclick={continueWithType}
					>
						<span>{t('common.continue')}</span>
						<img src={nextIcon} alt="" width="34" height="34" />
					</button>
				</div>
			{:else}
				<div class="form-container">
					{#if matcher === 'final'}
						<p class="field-hint">
							{t('rules.everythingElse')}. {t('rules.finalDetails')}
						</p>
						{@render serverField()}
					{:else if matcher === 'domain'}
						<DesignTextField
							id="domain-input"
							label={t('rules.domainLink')}
							prefix="https://"
							placeholder="example.com"
							bind:value={name}
							invalid={!!error}
							oninput={() => (error = '')}
							onpaste={handleDomainPaste}
						/>
						{@render serverField()}
					{:else if matcher === 'application'}
						<div class="field-group">
							<span class="field-label">{t('rules.applicationField')}</span>
							<button
								type="button"
								class="selector-pill"
								onclick={onSelectProcess}
								aria-label={t('rules.selectProcess')}
							>
								<span
									class="selector-value small"
									class:placeholder-val={!processes.length}
								>
									{#if processes.length > 1}
										<strong>{processes.length}</strong>
										<span class="muted">{t('rules.selectedCount')}</span>
									{:else}
										{processes[0] ?? t('rules.selectProcess')}
									{/if}
								</span>
								<img src={chevronIcon} alt="" class="chevron-right" />
							</button>
						</div>
						{@render serverField()}
					{:else}
						{@render serverField()}
						<DesignTextField
							id="geo-input"
							label={t('rules.geoCategory', {
								type: matcher === 'geosite' ? 'GeoSite' : 'GeoIP',
							})}
							prefix={matcher === 'geosite' ? 'geosite:' : 'geoip:'}
							placeholder={matcher === 'geosite' ? 'category-ru' : 'ru'}
							bind:value={name}
							invalid={!!error}
							oninput={() => (error = '')}
							onpaste={handleGeoPaste}
						/>
						<div class="geo-examples">
							<span class="example-label">{t('rules.examplesLabel')}</span>
							<div class="example-chips">
								{#each matcher === 'geosite' ? ['category-ru', 'antifilter', 'antifilter-community'] : ['ru', 'private', 'antifilter'] as ex (ex)}
									<button
										type="button"
										class="example-tag"
										class:active={name.trim().toLowerCase() === ex}
										onclick={() => {
											name = ex;
											error = '';
										}}
									>
										{capitalize(ex)}
									</button>
								{/each}
							</div>
						</div>
					{/if}

					{#if error}
						<p class="error-msg" role="alert">{error}</p>
					{/if}
				</div>

				<div class="design-action-bar">
					<IconButton
						design
						icon={backIcon}
						ariaLabel={t('common.back')}
						size="lg"
						variant="surface"
						onclick={goBack}
					/>
					<IconButton
						design
						icon={checkIcon}
						ariaLabel={t('common.save')}
						size="lg"
						variant="primary"
						disabled={!canSave}
						onclick={handleSave}
					/>
				</div>
			{/if}
		</div>
	{/key}
</section>

<style>
	.editor-step {
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		min-height: 0;
	}
	.rule-editor-view {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		height: 100%;
		min-height: 0;
		padding-top: 12px;
		box-sizing: border-box;
	}

	/* Step 1: rule type list. */
	.type-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		width: 100%;
		padding-bottom: 96px;
		overflow-y: auto;
	}
	.type-row {
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		width: 100%;
		min-height: 60px;
		padding: 16px 16px 16px 24px;
		border: 0;
		border-radius: 24px;
		background: var(--design-panel);
		color: #fff;
		text-align: left;
		transition:
			background-color 180ms ease,
			transform 160ms var(--ease-spring);
	}
	.type-row.selected {
		background: var(--design-panel-open);
	}
	.type-row:active {
		transform: scale(0.99);
	}
	.type-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 28px;
	}
	.type-label {
		font-family: 'Satoshi', var(--font-family-base);
		font-size: 16px;
		font-weight: 500;
		line-height: 21.6px;
		letter-spacing: -0.3px;
	}
	.type-radio {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: #ffffff0a;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.type-dot {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 2px 7.7px #00000040;
	}
	.type-desc {
		padding: 10px 8px 8px 0;
		color: #ffffff80;
		font-size: 16px;
		line-height: 20px;
		letter-spacing: 0.2px;
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
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			background-color 180ms ease,
			color 180ms ease,
			transform 160ms var(--ease-spring);
	}
	.continue-btn:disabled {
		background: #262626;
		color: #ffffff4d;
		opacity: 1;
	}
	/* The chevron asset is dark; on the disabled pill it matches the dim label. */
	.continue-btn:disabled img {
		filter: brightness(0) invert(1);
		opacity: 0.3;
	}
	.continue-btn:active:not(:disabled) {
		transform: scale(0.97);
	}

	/* Step 2: form. */
	.form-container {
		display: flex;
		flex-direction: column;
		gap: 24px;
		width: 100%;
		padding: 0 9px 96px;
		overflow-y: auto;
		box-sizing: border-box;
	}
	.field-group {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
	}
	.field-label {
		padding: 0 8px;
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		line-height: 18.2px;
		letter-spacing: -0.3px;
	}
	.field-hint {
		margin: 0;
		padding: 0 8px;
		color: #ffffffb8;
		font-size: 13px;
		line-height: 1.4;
	}

	.selector-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: 60px;
		border: 0;
		border-radius: 24px;
		background: var(--design-panel-open);
		color: #fff;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		box-sizing: border-box;
		transition: background-color 180ms ease;
		justify-content: space-between;
		padding: 0 18px 0 14px;
		text-align: left;
	}
	.selector-pill:hover {
		background: #ffffff14;
	}

	.server-info {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
	}
	.server-badge {
		display: grid;
		place-items: center;
		flex: 0 0 32px;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: #ffffff1a;
	}
	.selector-value {
		overflow: hidden;
		color: #fff;
		font-size: 14px;
		line-height: 18.2px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.selector-value.small,
	.selector-value.placeholder-val {
		padding-left: 2px;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.2px;
	}
	.selector-value strong {
		margin-right: 4px;
		font-weight: 600;
	}
	.muted {
		color: #ffffff80;
	}
	.chevron-right {
		width: 24px;
		height: 24px;
		transform: rotate(-90deg);
		filter: brightness(0) invert(1);
	}

	.geo-examples {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.example-label {
		padding: 0 8px;
		color: #ffffff80;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.3px;
	}
	.example-chips {
		display: flex;
		flex-wrap: wrap;
		gap: 10px;
	}
	.example-tag {
		height: 42px;
		padding: 0 18px;
		border: 0;
		border-radius: 123px;
		background: #ffffff08;
		color: #ffffff80;
		font-size: 14px;
		line-height: 18.2px;
		transition:
			background-color 160ms ease,
			color 160ms ease;
	}
	.example-tag:hover,
	.example-tag.active {
		background: #ffffff1a;
		color: #fff;
	}

	.error-msg {
		margin: -12px 0 0;
		padding: 0 8px;
		color: #ff6c67;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.2px;
	}
</style>
