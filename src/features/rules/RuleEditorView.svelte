<script lang="ts">
	import nextIcon from '../../assets/chevron-right.svg';
	import type { Rule, RuleMatcher, Server } from '../../lib/services/catalog';
	import { t } from '../../lib/i18n';
	import { screen, morphIn, morphOut } from '../../lib/motion';
	import DesignTextField from '../../components/DesignTextField.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import backIcon from '../../assets/back.svg';
	import chevronIcon from '../../assets/chevron-down.svg';
	import checkIcon from '../../assets/check.svg';
	import './editor/rule-editor.css';
	import RuleTypeStep from './editor/RuleTypeStep.svelte';
	import ServerField from './editor/ServerField.svelte';
	import GeoField from './editor/GeoField.svelte';
	import {
		buildRules,
		canSave,
		cleanDomain,
		initialName,
		processList,
		splitGeoPrefix,
		type RuleIssue,
		type RuleTypeOption,
	} from './editor/ruleEditorModel';

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
		/** Offers the catch-all type. There is at most one. */
		allowFinal?: boolean;
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
		allowFinal = true,
	}: Props = $props();

	const ruleTypes: RuleTypeOption[] = [
		{ id: 'domain', label: 'rules.typeDomain', desc: 'rules.typeDomainDesc' },
		{
			id: 'application',
			label: 'rules.typeApplication',
			desc: 'rules.typeApplicationDesc',
		},
		{ id: 'geosite', label: 'GeoSite', desc: 'rules.typeGeositeDesc' },
		{ id: 'geoip', label: 'GeoIP', desc: 'rules.typeGeoipDesc' },
		{ id: 'final', label: 'rules.everythingElse', desc: 'rules.typeFinalDesc' },
	];
	const shownTypes = $derived(
		allowFinal ? ruleTypes : ruleTypes.filter((type) => type.id !== 'final'),
	);
	let pickedType = $state<RuleMatcher | null>(null);

	// svelte-ignore state_referenced_locally
	let matcher = $state<RuleMatcher>(
		rule.matcher ?? (rule.isDomain ? 'domain' : 'application'),
	);

	// svelte-ignore state_referenced_locally
	let name = $state(initialName(rule, matcher));
	let serverId = $derived(rule.serverId);
	let error = $state('');

	$effect(() => {
		onNameChange?.(name);
	});

	let processes = $derived(processList(name));
	let saveEnabled = $derived(canSave(matcher, name));

	function continueWithType() {
		if (!pickedType) return;
		if (pickedType !== matcher) {
			switchMatcher(pickedType);
			name = '';
		}
		onStepChange?.('form');
	}

	// Back from the form returns to the type list only for a new rule. An
	// existing rule opens on the form, so back leaves the editor.
	// svelte-ignore state_referenced_locally
	const startedOnType = step === 'type';

	function goBack() {
		if (step === 'form' && startedOnType && onStepChange) {
			pickedType = matcher;
			onStepChange('type');
		} else {
			onCancel();
		}
	}

	function issueMessage(issue: RuleIssue): string {
		switch (issue.kind) {
			case 'invalidDomain':
				return t('errors.invalidRule');
			case 'chooseProcess':
				return t('rules.chooseProcessError') || 'Выберите процесс';
			case 'invalidGeoTag':
				return (
					t('rules.invalidGeoTag') ||
					'Недопустимый тег. Используйте латинские буквы, цифры, _, -, !, @ (до 128 символов)'
				);
			case 'mismatchedPrefix':
				return (
					t('rules.mismatchedPrefixError', {
						expected: issue.expected,
						wrong: issue.wrong,
					}) ||
					`Неверный префикс: для ${issue.matcher} используйте ${issue.expected}`
				);
		}
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
		const result = splitGeoPrefix(pasted, matcher);
		if ('issue' in result) {
			error = issueMessage(result.issue);
			return;
		}
		name = result.value;
		error = '';
	}

	function switchMatcher(next: RuleMatcher) {
		matcher = next;
		onMatcherChange(next);
		error = '';
	}

	function handleSave() {
		error = '';
		const result = buildRules(rule, matcher, name, serverId);
		if ('issue' in result) {
			error = issueMessage(result.issue);
			return;
		}
		onSave(result.rules);
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

<section class="rule-editor-view" aria-label={t('rules.editTitle')}>
	{#key step}
		<div class="editor-step" in:screen={{ dir: step === 'form' ? 1 : -1 }}>
			{#if step === 'type'}
				<RuleTypeStep
					types={shownTypes}
					bind:picked={pickedType}
					onConfirm={continueWithType}
				/>
			{:else}
				<div class="form-container">
					{#if matcher === 'final'}
						<p class="field-hint">{t('rules.finalHint')}</p>
						<ServerField
							{serverId}
							{server}
							{subscriptionName}
							onSelect={onSelectServer}
						/>
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
						<ServerField
							{serverId}
							{server}
							{subscriptionName}
							onSelect={onSelectServer}
						/>
					{:else if matcher === 'application'}
						<div class="field-group">
							<span class="field-label">{t('rules.applicationField')}</span>
							<button
								type="button"
								class="selector-pill"
								class:chosen={processes.length > 0}
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
						<ServerField
							{serverId}
							{server}
							{subscriptionName}
							onSelect={onSelectServer}
						/>
					{:else}
						<ServerField
							{serverId}
							{server}
							{subscriptionName}
							onSelect={onSelectServer}
						/>
						<GeoField
							{matcher}
							bind:name
							invalid={!!error}
							onEdit={() => (error = '')}
							onPaste={handleGeoPaste}
						/>
					{/if}

					{#if error}
						<p class="error-msg" role="alert">{error}</p>
					{/if}
				</div>
			{/if}
		</div>
	{/key}

	<!-- The bar stays while the steps change: "Continue" shrinks into the check. -->
	<div class="design-action-bar">
		<IconButton
			design
			icon={backIcon}
			ariaLabel={t('common.back')}
			size="lg"
			variant="surface"
			onclick={goBack}
		/>
		<button
			type="button"
			class="primary-morph"
			class:wide={step === 'type'}
			disabled={step === 'type' ? !pickedType : !saveEnabled}
			aria-label={step === 'type' ? t('common.continue') : t('common.save')}
			onclick={step === 'type' ? continueWithType : handleSave}
		>
			{#key step}
				<span
					class="primary-layer"
					class:pill={step === 'type'}
					in:morphIn
					out:morphOut
				>
					{#if step === 'type'}
						<span>{t('common.continue')}</span>
						<img src={nextIcon} alt="" width="34" height="34" />
					{:else}
						<img src={checkIcon} alt="" width="34" height="34" />
					{/if}
				</span>
			{/key}
		</button>
	</div>
</section>
