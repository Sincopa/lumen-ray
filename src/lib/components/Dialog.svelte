<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { t } from '../i18n';
  let {
    title,
    onClose,
    children,
  }: { title: string; onClose: () => void; children: Snippet } = $props();
  let dialog: HTMLDialogElement;
  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    return () => {
      dialog.close();
      previous?.focus();
    };
  });
</script>

<dialog
  bind:this={dialog}
  oncancel={(e) => {
    e.preventDefault();
    onClose();
  }}
  aria-label={title}
>
  <header>
    <h2>{title}</h2>
    <button type="button" onclick={onClose} aria-label={t('common.close')}
      >×</button
    >
  </header>
  {@render children()}
</dialog>

<style>
  dialog {
    width: min(360px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    overflow: auto;
    border: 1px solid #ffffff29;
    border-radius: 30px;
    padding: 22px;
    background: #151516;
    color: #fff;
    box-shadow: 0 24px 70px #000b;
  }
  dialog::backdrop {
    background: #000b;
    backdrop-filter: blur(10px);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 18px;
  }
  h2 {
    font-size: 20px;
    line-height: 1.2;
    letter-spacing: -0.4px;
    margin: 0;
  }
  button {
    width: 36px;
    height: 36px;
    min-height: 0;
    flex-shrink: 0;
    border: 1px solid #ffffff1b;
    border-radius: 50%;
    background: #ffffff10;
    padding: 0;
    font-size: 23px;
    line-height: 1;
  }
</style>
