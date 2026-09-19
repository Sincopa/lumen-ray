<script lang="ts">
  import addIcon from '../../../assets/add.svg';
  import { t } from '../../i18n';

  interface Props {
    onAddProfile: () => void;
  }

  let { onAddProfile }: Props = $props();
</script>

<div class="empty-home-view">
  <button class="add-profile-btn" onclick={onAddProfile} type="button">
    <img src={addIcon} alt="" class="plus-icon" />
    <span class="btn-label">
      {t('subscriptions.addProfile') || 'Добавьте профиль'}
    </span>
  </button>
</div>

<style>
  .empty-home-view {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    flex: 1;
    min-height: 0;
    user-select: none;
  }

  .add-profile-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 56px;
    padding: 0 28px;
    border-radius: 32px;
    background: rgba(0, 0, 0, 0.22);
    border: none;
    -webkit-backdrop-filter: blur(16px);
    backdrop-filter: blur(16px);
    color: #ffffff;
    cursor: pointer;
    box-sizing: border-box;
    transition:
      background-color 200ms ease,
      transform 200ms var(--ease-spring),
      box-shadow 200ms ease;
  }

  .add-profile-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    transform: scale(1.04);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.35);
  }

  .add-profile-btn:active {
    transform: scale(0.96);
  }

  .plus-icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    filter: brightness(0) invert(1);
  }

  .btn-label {
    font-size: 18px;
    font-weight: 500;
    letter-spacing: -0.2px;
    color: #ffffff;
  }
</style>
