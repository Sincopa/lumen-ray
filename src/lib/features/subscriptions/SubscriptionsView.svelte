<script lang="ts">
  import { slide } from 'svelte/transition';
  import { catalog, connection } from '../../services';
  import { errorCode } from '../../services/errors';
  import { t } from '../../i18n';
  import type { Subscription } from '../../services/catalog';
  import Dialog from '../../components/Dialog.svelte';
  import reloadIcon from '../../../assets/reload.svg';
  import menuIcon from '../../../assets/menu.svg';
  import pencilIcon from '../../../assets/pencil.svg';
  import trashIcon from '../../../assets/trash.svg';
  import shareIcon from '../../../assets/share.svg';
  import chevronIcon from '../../../assets/chevron.svg';

  const catalogStore = catalog.state;
  const connectionStore = connection.state;

  // Local state
  let expanded = $state<string | null>('grbnk');
  let editing = $state<Subscription | null>(null);
  let deleting = $state<Subscription | null>(null);
  let menu = $state<string | null>(null);
  let name = $state('');
  let busy = $state<string | null>(null);
  let error = $state('');
  let exported = $state('');

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

<section class="subscriptions-view" aria-label={t('subscriptions.title')}>
  {#if error}<p role="alert">{error}</p>{/if}
  <div class="cards-list">
    {#each $catalogStore.subscriptions as sub (sub.id)}
      <div class="sub-card-wrapper">
        <article class="sub-card" class:expanded={expanded === sub.id}>
          <div class="card-header">
            <button
              class="title-group"
              aria-expanded={expanded === sub.id}
              onclick={() => (expanded = expanded === sub.id ? null : sub.id)}
            >
              <span class="sub-name">{sub.name}</span>
              <span
                class="status-indicator"
                class:expired={sub.status === 'expired'}
                aria-hidden="true"
              ></span>
            </button>
            <div class="actions-group">
              <button
                class="action-icon-btn"
                aria-label={t('common.reload')}
                disabled={busy !== null}
                onclick={() => refresh(sub.id)}
              >
                <img src={reloadIcon} class="btn-icon" alt="" />
              </button>
              <button
                class="action-icon-btn"
                class:active-menu-btn={menu === sub.id}
                aria-label={t('common.menu')}
                aria-expanded={menu === sub.id}
                onclick={() => (menu = menu === sub.id ? null : sub.id)}
              >
                <img src={menuIcon} class="btn-icon" alt="" />
              </button>
              <button
                class="action-icon-btn card-chevron"
                class:expanded={expanded === sub.id}
                aria-label={sub.name}
                aria-expanded={expanded === sub.id}
                onclick={() => (expanded = expanded === sub.id ? null : sub.id)}
              >
                <img src={chevronIcon} class="btn-icon" alt="" />
              </button>
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
                {sub.updatedMinutesAgo !== undefined
                  ? t('subscriptions.updatedMinutesAgo', {
                      count: sub.updatedMinutesAgo,
                    })
                  : sub.updatedAt
                    ? t('subscriptions.updatedAt', {
                        time: new Date(sub.updatedAt).toLocaleString(),
                      })
                    : t('demo.sample')}
              </div>
            </div>
          {/if}
        </article>
        {#if menu === sub.id}
          <div class="popup-menu">
            <button
              class="menu-item"
              onclick={() => {
                editing = sub;
                name = sub.name;
                menu = null;
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={pencilIcon} alt="" />
              </span>
              <span>{t('common.edit')}</span>
            </button>
            <button
              class="menu-item"
              disabled={busy !== null}
              onclick={() => {
                menu = null;
                void refresh(sub.id);
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={reloadIcon} alt="" />
              </span>
              <span>{t('common.reload')}</span>
            </button>
            <button class="menu-item" onclick={() => exportSub(sub)}>
              <span class="menu-item-icon-circle">
                <img src={shareIcon} alt="" />
              </span>
              <span>{t('common.export')}</span>
            </button>
            <button
              class="menu-item danger"
              disabled={$connectionStore.busy ||
                $connectionStore.status === 'connected'}
              onclick={() => {
                deleting = sub;
                menu = null;
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={trashIcon} alt="" />
              </span>
              <span>{t('common.delete')}</span>
            </button>
          </div>
        {/if}
      </div>
    {:else}<p>{t('subscriptions.empty')}</p>{/each}
  </div>
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
  }
  .cards-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    overflow-y: auto;
    min-height: 0;
    padding-bottom: 220px;
  }
  .sub-card-wrapper {
    position: relative;
    width: 100%;
    animation: surface-in 360ms var(--ease-spring) both;
  }
  .sub-card-wrapper:nth-child(2) {
    animation-delay: 45ms;
  }
  .sub-card-wrapper:nth-child(3) {
    animation-delay: 90ms;
  }
  .sub-card {
    background: rgba(0, 0, 0, 0.22);
    border: 1px solid #ffffff14;
    border-radius: 32px;
    padding: 18px 22px;
    display: flex;
    flex-direction: column;
    backdrop-filter: blur(14px);
  }
  .sub-card:hover,
  .sub-card.expanded {
    background: rgba(0, 0, 0, 0.3);
    border-color: #ffffff24;
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
  .status-indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #3aee29;
    box-shadow: 0 0 8px #3aee29a6;
    flex-shrink: 0;
  }
  .status-indicator.expired {
    background: #ef3340;
    box-shadow: 0 0 8px #ef334099;
  }
  .actions-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .action-icon-btn {
    width: 28px;
    height: 28px;
    min-height: 0;
    border-radius: 50%;
    background: none;
    border: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }
  .action-icon-btn:hover,
  .active-menu-btn {
    background: #ffffff20;
  }
  .btn-icon {
    width: 18px;
    height: 18px;
    filter: brightness(0) invert(1);
    opacity: 0.75;
  }
  .card-chevron {
    transition: transform 0.28s;
  }
  .card-chevron.expanded {
    transform: rotate(180deg);
  }
  .card-body {
    margin-top: 12px;
    padding-top: 11px;
    border-top: 1px solid #ffffff0f;
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
  .popup-menu {
    position: absolute;
    top: 52px;
    right: 12px;
    width: 197px;
    background: #18181a;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 24px;
    padding: 10px;
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    gap: 4px;
    z-index: 50;
    transform-origin: top right;
    animation: menu-in 180ms var(--ease-spring) both;
  }
  .menu-item {
    width: 100%;
    height: 40px;
    min-height: 0;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.04);
    border: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 8px;
    text-align: left;
    font-size: 13px;
    font-weight: 500;
    transition: background 0.18s ease;
  }
  .menu-item:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .menu-item.danger:hover {
    background: rgba(243, 57, 57, 0.2);
    color: #ff8080;
  }
  .menu-item-icon-circle {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: #ffffff;
  }
  .menu-item-icon-circle img {
    width: 15px;
    height: 15px;
    filter: brightness(0) invert(1);
  }
  @keyframes surface-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.96);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .card-chevron {
      transition: none;
    }
  }
</style>
