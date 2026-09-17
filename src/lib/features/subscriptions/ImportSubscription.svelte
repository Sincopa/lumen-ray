<script lang="ts">
  import Dialog from '../../components/Dialog.svelte';
  import { catalog } from '../../services';
  import { errorCode } from '../../services/errors';
  import { t } from '../../i18n';
  let { onClose }: { onClose: () => void } = $props();
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
      await catalog.importSubscription(source, name);
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
  <form onsubmit={submit}>
    <p>{t('demo.import')}</p>
    <label
      >{t('common.name')}<input
        bind:value={name}
        required
        maxlength="80"
        autocomplete="off"
      /></label
    >
    <label
      >{t('subscriptions.source')}<textarea
        bind:value={source}
        required
        maxlength="65536"
        autocomplete="off"
        spellcheck="false"></textarea></label
    >
    {#if error}<p role="alert">{error}</p>{/if}
    <button type="submit" disabled={busy}>{t('common.add')}</button>
  </form>
</Dialog>
