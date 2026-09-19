<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    description?: string;
    class?: string;
    children: Snippet;
  }

  let { label, description, class: className = '', children }: Props = $props();
</script>

<div class="setting-item {className}">
  <div class="item-text">
    <span class="label">{label}</span>
    {#if description}
      <span class="desc">{description}</span>
    {/if}
  </div>
  {@render children()}
</div>

<style>
  .setting-item {
    background: rgba(0, 0, 0, 0.25);
    border: none;
    border-radius: 20px;
    padding: 16px 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    -webkit-backdrop-filter: blur(12px);
    backdrop-filter: blur(12px);
    box-sizing: border-box;
  }

  .item-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .label {
    font-family: var(--font-family-base);
    font-size: 14px;
    font-weight: 500;
    color: #ffffff;
  }

  .desc {
    font-family: var(--font-family-base);
    font-size: 13px;
    color: #c7bcd0;
  }
</style>
