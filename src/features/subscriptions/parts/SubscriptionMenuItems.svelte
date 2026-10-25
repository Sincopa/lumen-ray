<script lang="ts">
	import pencilIcon from '../../../assets/pencil.svg';
	import trashIcon from '../../../assets/trash.svg';
	import shareIcon from '../../../assets/share-link.svg';
	import { t } from '../../../lib/i18n';
	import type { Subscription } from '../../../lib/services/catalog';
	import MenuItem from '../../../components/MenuItem.svelte';

	interface Props {
		sub: Subscription;
		deleteDisabled: boolean;
		onEdit: (sub: Subscription) => void;
		onShare: (sub: Subscription) => void;
		onDelete: (sub: Subscription) => void;
	}
	let { sub, deleteDisabled, onEdit, onShare, onDelete }: Props = $props();
</script>

<MenuItem
	icon={pencilIcon}
	label={t('common.edit')}
	onclick={() => onEdit(sub)}
/>
<MenuItem
	icon={shareIcon}
	label={t('common.share')}
	onclick={() => onShare(sub)}
/>
<MenuItem
	icon={trashIcon}
	label={t('common.delete')}
	variant="danger"
	disabled={deleteDisabled}
	onclick={() => onDelete(sub)}
/>
