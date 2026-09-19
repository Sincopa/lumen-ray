<script lang="ts">
  import { slide } from 'svelte/transition';
  import { catalog, connection } from '../../services';
  import { errorCode } from '../../services/errors';
  import { t, i18n } from '../../i18n';
  import type { Subscription } from '../../services/catalog';
  import Dialog from '../../components/Dialog.svelte';
  import StatusDot from '../../components/StatusDot.svelte';
  import IconButton from '../../components/IconButton.svelte';
  import PopupMenu from '../../components/PopupMenu.svelte';
  import MenuItem from '../../components/MenuItem.svelte';
  import reloadIcon from '../../../assets/reload.svg';
  import menuIcon from '../../../assets/menu.svg';
  import pencilIcon from '../../../assets/pencil.svg';
  import trashIcon from '../../../assets/trash.svg';
  import shareIcon from '../../../assets/share.svg';
  import chevronIcon from '../../../assets/chevron.svg';
  import { formatSubscriptionUpdatedAgo } from '../../utils/time';

  const catalogStore = catalog.state;
  const connectionStore = connection.state;

  // Local state
  let expanded = $state<string | null>(null);
  let editing = $state<Subscription | null>(null);
  let deleting = $state<Subscription | null>(null);
  let menu = $state<string | null>(null);
  let name = $state('');
  let busy = $state<string | null>(null);
  let error = $state('');
  let exported = $state('');

  let activeSubId = $derived(
    $catalogStore.selectedSubscriptionId ??
      $catalogStore.servers.find((s) => s.id === $catalogStore.selectedServerId)
        ?.subscriptionId ??
      $catalogStore.subscriptions[0]?.id,
  );

  let menuTop = $state(68);
  let viewEl = $state<HTMLElement | null>(null);

  let activeMenuSub = $derived(
    $catalogStore.subscriptions.find((s) => s.id === menu),
  );

  function formatUpdatedAgo(sub: Subscription): string {
    return formatSubscriptionUpdatedAgo(
      sub.updatedAt,
      sub.updatedMinutesAgo,
      i18n.currentLanguage,
    );
  }

  function toggleMenu(subId: string, e: MouseEvent) {
    if (menu === subId) {
      menu = null;
      return;
    }
    const btn = e.currentTarget as HTMLElement | null;
    if (btn && viewEl) {
      const btnRect = btn.getBoundingClientRect();
      const viewRect = viewEl.getBoundingClientRect();
      menuTop = Math.round(btnRect.bottom - viewRect.top + 8);
    }
    menu = subId;
  }

  // Action handlers
  async function refresh(id: string) {
    if (busy) return;
    busy = id;
    error = '';
    try {
      await catalog.refreshSubscription(id);
    } catch (e) {
      error = t('errors.' + errorCode(e));
    } finally {
      busy = null;
    }
  }

  function rename(e: SubmitEvent) {
    e.preventDefault();
    if (!editing) return;
    try {
      catalog.renameSubscription(editing.id, name);
      editing = null;
      error = '';
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }

  function remove() {
    if (!deleting) return;
    try {
      catalog.removeSubscription(deleting.id);
      deleting = null;
      error = '';
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }

  function exportSub(sub: Subscription) {
    exported = JSON.stringify(
      {
        version: 1,
        demo: true,
        name: sub.name,
        servers: $catalogStore.servers.filter(
          (s) => s.subscriptionId === sub.id,
        ),
      },
      null,
      2,
    );
    menu = null;
  }
</script>

<svelte:window
  onclick={(e) => {
    if (menu) {
      const target = e.target as HTMLElement | null;
      if (!target?.closest('.popup-menu') && !target?.closest('.icon-btn')) {
        menu = null;
      }
    }
  }}
/>

<section
  class="subscriptions-view"
  bind:this={viewEl}
  aria-label={t('subscriptions.title')}
>
  {#if error}<p role="alert">{error}</p>{/if}
  <div class="cards-list" onscroll={() => (menu = null)}>
    {#each $catalogStore.subscriptions as sub (sub.id)}
      <div class="sub-card-wrapper" class:has-open-menu={menu === sub.id}>
        <article class="sub-card" class:expanded={expanded === sub.id}>
          <div class="card-header">
            <button
              class="title-group"
              aria-expanded={expanded === sub.id}
              onclick={() => {
                catalog.selectSubscription(sub.id);
                expanded = expanded === sub.id ? null : sub.id;
              }}
            >
              <span class="sub-name">{sub.name}</span>
              {#if activeSubId === sub.id}
                <StatusDot variant="active" />
              {/if}
            </button>
            <div class="actions-group">
              <IconButton
                icon={reloadIcon}
                ariaLabel={t('common.reload')}
                disabled={busy !== null}
                onclick={() => refresh(sub.id)}
              />
              <IconButton
                icon={menuIcon}
                ariaLabel={t('common.menu')}
                active={menu === sub.id}
                expanded={menu === sub.id}
                onclick={(e) => toggleMenu(sub.id, e)}
              />
              <IconButton
                icon={chevronIcon}
                ariaLabel={sub.name}
                expanded={expanded === sub.id}
                class="card-chevron"
                onclick={() => {
                  catalog.selectSubscription(sub.id);
                  expanded = expanded === sub.id ? null : sub.id;
                }}
              />
            </div>
          </div>
          {#if expanded === sub.id}
            <div class="card-body" transition:slide={{ duration: 240 }}>
              <div class="metrics-row">
                <div class="metric-item">
                  <span class="metric-label">
                    {t('subscriptions.daysLabel')}
                  </span>
                  <span class="metric-val">{sub.daysRemaining ?? '—'}</span>
                </div>
                <div class="metric-item">
                  <span class="metric-label">
                    {t('subscriptions.trafficLabel')}
                  </span>
                  <span class="metric-val">{sub.traffic ?? '—'}</span>
                </div>
              </div>
              <div class="updated-time">
                {formatUpdatedAgo(sub)}
              </div>
            </div>
          {/if}
        </article>
      </div>
    {:else}<p>{t('subscriptions.empty')}</p>{/each}
  </div>

  {#if menu && activeMenuSub}
    <PopupMenu top="{menuTop}px" right="20px">
      <MenuItem
        icon={pencilIcon}
        label={t('common.edit')}
        onclick={() => {
          editing = activeMenuSub;
          name = activeMenuSub?.name ?? '';
          menu = null;
        }}
      />
      <MenuItem
        icon={reloadIcon}
        label={t('common.reload')}
        disabled={busy !== null}
        onclick={() => {
          const id = activeMenuSub?.id;
          menu = null;
          if (id) void refresh(id);
        }}
      />
      <MenuItem
        icon={shareIcon}
        label={t('common.export')}
        onclick={() => {
          if (activeMenuSub) exportSub(activeMenuSub);
        }}
      />
      <MenuItem
        icon={trashIcon}
        label={t('common.delete')}
        variant="danger"
        disabled={$connectionStore.busy ||
          $connectionStore.status === 'connected'}
        onclick={() => {
          deleting = activeMenuSub;
          menu = null;
        }}
      />
    </PopupMenu>
  {/if}
</section>
{#if editing}
  <Dialog title={t('common.edit')} onClose={() => (editing = null)}>
    <form onsubmit={rename}>
      <label>
        {t('common.name')}
        <input bind:value={name} maxlength="80" required />
      </label>
      {#if error}
        <p role="alert">{error}</p>
      {/if}
      <button type="submit">{t('common.save')}</button>
    </form>
  </Dialog>
{/if}

{#if deleting}
  <Dialog title={t('common.delete')} onClose={() => (deleting = null)}>
    <p>{t('subscriptions.deleteConfirm', { name: deleting.name })}</p>
    {#if error}
      <p role="alert">{error}</p>
    {/if}
    <button class="danger" onclick={remove}>{t('common.delete')}</button>
  </Dialog>
{/if}

{#if exported}
  <Dialog title={t('common.export')} onClose={() => (exported = '')}>
    <p>{t('demo.export')}</p>
    <textarea
      readonly
      value={exported}
      aria-label={t('common.export')}
      rows="10"></textarea>
  </Dialog>
{/if}

<style>
  .subscriptions-view {
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    min-height: 0;
    gap: 12px;
    padding-top: 0;
    position: relative;
  }
  .cards-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
    padding-bottom: 96px;
    scrollbar-width: none;
    -ms-overflow-style: none;
  }
  .cards-list::-webkit-scrollbar {
    display: none;
  }
  .sub-card-wrapper {
    position: relative;
    width: 100%;
  }
  .sub-card-wrapper.has-open-menu {
    z-index: 50;
  }
  .sub-card {
    background: rgba(0, 0, 0, 0.22);
    border: none;
    border-radius: 32px;
    padding: 18px 22px;
    display: flex;
    flex-direction: column;
  }
  .sub-card:hover,
  .sub-card.expanded {
    background: rgba(0, 0, 0, 0.3);
    border: none;
  }
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    height: 24px;
    gap: 8px;
  }
  .title-group {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    text-align: left;
    flex: 1;
  }
  .sub-name {
    font-size: 15px;
    font-weight: 500;
    letter-spacing: -0.2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  :global(.card-chevron) {
    transition: transform 0.28s;
  }
  :global(.card-chevron.expanded) {
    transform: rotate(180deg);
  }
  .card-body {
    margin-top: 12px;
    padding-top: 11px;
    border-top: none;
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 12px;
  }
  .metrics-row {
    display: flex;
    gap: 24px;
  }
  .metric-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .metric-label {
    font-size: 10px;
    color: #ffffff80;
  }
  .metric-val {
    font-size: 13px;
    font-weight: 600;
  }
  .updated-time {
    font-size: 10px;
    color: #ffffff8c;
    max-width: 50%;
    text-align: right;
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.card-chevron) {
      transition: none;
    }
  }
</style>
