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
    <button onclick={onClose} aria-label={t('common.close')}>×</button>
  </header>
  {@render children()}
</dialog>

<style>
  dialog {
    width: min(360px, calc(100vw - 32px));
    max-height: calc(100dvh - 48px);
    overflow: auto;
    border: 1px solid #655675;
    border-radius: 24px;
    padding: 20px;
    background: #18121f;
    color: #fff;
  }
  dialog::backdrop {
    background: #000a;
    backdrop-filter: blur(6px);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  h2 {
    font-size: 18px;
    margin: 0 0 16px;
  }
  button {
    width: 44px;
    height: 44px;
  }
</style>
