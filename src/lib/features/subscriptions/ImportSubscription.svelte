<script lang="ts">
	import Dialog from '../../components/Dialog.svelte';
	import { catalog } from '../../services';
	import { errorCode } from '../../services/errors';
	import { t } from '../../i18n';

	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	let name = $state('');
	let source = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (busy) return;
		busy = true;
		error = '';
		try {
			await catalog.importSubscription(source, name.trim() || undefined);
			source = '';
			onClose();
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			busy = false;
		}
	}
</script>

<Dialog title={t('subscriptions.addSubscription')} {onClose}>
	<form class="panel-form" onsubmit={submit}>
		<p class="form-intro">{t('subscriptions.addHint')}</p>

		<label>
			{t('common.name')}
			<input
				bind:value={name}
				maxlength="80"
				autocomplete="off"
				placeholder={t('subscriptions.namePlaceholder')}
			/>
		</label>

		<label>
			{t('subscriptions.source')}
			<textarea
				bind:value={source}
				required
				maxlength="65536"
				autocomplete="off"
				spellcheck="false"
				rows="3"
				placeholder={t('subscriptions.sourcePlaceholder')}></textarea>
		</label>

		<p class="field-hint">{t('demo.import')}</p>

		{#if error}
			<p class="form-error" role="alert">{error}</p>
		{/if}

		<button class="form-submit" type="submit" disabled={busy}>
			{t('subscriptions.addSubscription')}
		</button>
	</form>
</Dialog>
