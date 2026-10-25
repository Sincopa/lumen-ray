<script lang="ts">
	import { catalog } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { t } from '../../lib/i18n';
	import IconButton from '../../components/IconButton.svelte';
	import DesignTextField from '../../components/DesignTextField.svelte';
	import backIcon from '../../assets/back.svg';
	import checkIcon from '../../assets/check.svg';

	interface Props {
		onClose: () => void;
		onSuccess?: () => void;
		/** Link handed over by a `lumen://import` deep link. */
		initialSource?: string;
	}

	let { onClose, onSuccess, initialSource = '' }: Props = $props();

	let name = $state('');
	let source = $state('');

	// The field shows an https:// prefix, so it is left out of the value.
	const stripHttps = (value: string) => value.replace(/^\s*https:\/\//i, '');

	$effect(() => {
		if (!initialSource) return;
		source = stripHttps(initialSource);
	});
	let error = $state('');
	let busy = $state(false);

	async function submit() {
		if (busy) return;
		let cleanSource = source
			.trim()
			.replace(/^\uFEFF/, '')
			.replace(/^["'<]|["'>]$/g, '')
			.trim();
		if (!cleanSource) {
			error = t('errors.invalidSubscription');
			return;
		}
		if (
			!cleanSource.includes('://') &&
			!cleanSource.startsWith('{') &&
			!cleanSource.startsWith('[') &&
			!cleanSource.includes('proxies:') &&
			cleanSource.includes('.') &&
			!/\s/.test(cleanSource)
		) {
			cleanSource = `https://${cleanSource}`;
		}
		busy = true;
		error = '';
		try {
			await catalog.importSubscription(cleanSource, name.trim() || undefined);
			source = '';
			name = '';
			onSuccess?.();
			onClose();
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			busy = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			void submit();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section class="add-profile-view" aria-label={t('subscriptions.addProfile')}>
	<div class="form-container">
		<DesignTextField
			id="profile-name"
			label={t('common.name')}
			placeholder={t('common.name')}
			bind:value={name}
			maxlength={80}
			spellcheck
		/>
		<DesignTextField
			id="profile-domain-link"
			label={t('subscriptions.profileLink')}
			prefix="https://"
			placeholder={t('subscriptions.domainLinkPlaceholder')}
			bind:value={source}
			invalid={!!error}
			oninput={() => {
				source = stripHttps(source);
				error = '';
			}}
		/>
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
			onclick={onClose}
		/>
		<IconButton
			design
			icon={checkIcon}
			ariaLabel={t('common.save')}
			size="lg"
			variant="primary"
			disabled={busy || !source.trim()}
			onclick={submit}
		/>
	</div>
</section>

<style>
	.add-profile-view {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		height: 100%;
		min-height: 0;
		padding-top: 24px;
		box-sizing: border-box;
	}
	.form-container {
		display: flex;
		flex-direction: column;
		gap: 24px;
		width: 100%;
		padding: 0 9px 96px;
		overflow-y: auto;
		box-sizing: border-box;
	}
	/* Provider errors are red text under the field. */
	.error-msg {
		margin: -6px 0 0;
		padding: 0 8px;
		color: #ff6c67;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.2px;
	}
</style>
