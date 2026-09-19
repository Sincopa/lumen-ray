<script lang="ts">
  import type { Snippet } from 'svelte';
  import StatusDot from './StatusDot.svelte';

  interface Props {
    title: string;
    showStatus?: boolean;
    statusVariant?: 'active' | 'expired';
    asHeader?: boolean;
    actions?: Snippet;
    children?: Snippet;
  }

  let {
    title,
    showStatus = true,
    statusVariant = 'active',
    asHeader = false,
    actions,
    children,
  }: Props = $props();
</script>

{#if asHeader}
  <header class="active-sub-card">
    <div class="header-card-title">
      <span class="title-text">{title}</span>
      {#if showStatus}
        <StatusDot variant={statusVariant} />
      {/if}
    </div>
    {#if actions}
      <div class="header-card-actions">
        {@render actions()}
      </div>
    {/if}
    {@render children?.()}
  </header>
{:else}
  <div class="active-sub-card">
    <div class="header-card-title">
      <span class="title-text">{title}</span>
      {#if showStatus}
        <StatusDot variant={statusVariant} />
      {/if}
    </div>
    {#if actions}
      <div class="header-card-actions">
        {@render actions()}
      </div>
    {/if}
    {@render children?.()}
  </div>
{/if}

<style>
  .active-sub-card {
    height: 62px;
    padding: 0 12px 0 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: #00000038;
    border-radius: 124px;
    /* backdrop-filter: blur(14px); */
    /* -webkit-backdrop-filter: blur(14px); */
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    gap: 8px;
  }

  .header-card-title {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    font-size: 15px;
    font-weight: 500;
    letter-spacing: -0.2px;
  }

  .title-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .header-card-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
</style>
