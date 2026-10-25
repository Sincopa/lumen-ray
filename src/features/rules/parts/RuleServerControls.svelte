<script lang="ts">
	import tunIcon from '../../../assets/mode-rules.svg';
	import type {
		Rule,
		RuleMatcher,
		Server,
	} from '../../../lib/services/catalog';
	import { pop } from '../../../lib/motion';
	import CountryFlag from '../../../components/CountryFlag.svelte';
	import MaterialIcon from '../../../components/MaterialIcon.svelte';
	import RoundCheckbox from '../../../components/RoundCheckbox.svelte';
	import ToggleSwitch from '../../../components/ToggleSwitch.svelte';
	import { t } from '../../../lib/i18n';

	interface Props {
		rule: Rule;
		matcher: RuleMatcher;
		server?: Server;
		serverName: string;
		isSub: boolean;
		selectMode: boolean;
		checked: boolean;
		onPickServer: () => void;
		onToggle: () => void;
		onSelect: () => void;
	}
	let {
		rule,
		matcher,
		server,
		serverName,
		isSub,
		selectMode,
		checked,
		onPickServer,
		onToggle,
		onSelect,
	}: Props = $props();

	const isDirect = $derived(rule.serverId === 'direct');
	const isBlock = $derived(rule.serverId === 'block');
</script>

<div class="rule-right">
	<button
		class="server-pill"
		title={serverName}
		onclick={onPickServer}
		aria-label={rule.name + ': ' + t('servers.title') + ' — ' + serverName}
	>
		{#if isDirect}
			<span class="flag-sm">
				<MaterialIcon name="alt_route" size={16} />
			</span>
		{:else if isBlock}
			<span class="flag-sm">
				<MaterialIcon name="block" size={16} />
			</span>
		{:else if isSub}
			<span class="flag-sm">
				<img src={tunIcon} alt="" width="24" height="24" />
			</span>
		{:else if server}
			<span class="flag-sm">
				<CountryFlag countryCode={server.countryCode} size={24} />
			</span>
		{/if}
		<span class="server-pill-name">
			{isSub && matcher !== 'application' ? 'Tun' : serverName}
		</span>
	</button>
	{#if selectMode}
		<span class="row-control" in:pop>
			<RoundCheckbox
				{checked}
				ariaLabel={t('rules.select', { name: rule.name })}
				onclick={onSelect}
			/>
		</span>
	{:else}
		<span class="row-control" in:pop>
			<ToggleSwitch
				checked={rule.enabled}
				mode="switch"
				ariaLabel={t('rules.toggle', { name: rule.name })}
				onchange={onToggle}
			/>
		</span>
	{/if}
</div>

<style>
	.rule-right {
		display: flex;
		align-items: center;
		gap: 16px;
		flex-shrink: 0;
	}
	.row-control {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
	}
	.server-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 32px;
		min-height: 0;
		background: var(--design-panel);
		border: none;
		outline: none;
		box-shadow: none;
		border-radius: 124px;
		padding: 4px 12px 4px 4px;
	}
	.server-pill:hover {
		background: #00000073;
	}
	.flag-sm {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.flag-sm img {
		display: block;
		filter: brightness(0) invert(1);
		opacity: 0.75;
	}
	.server-pill-name {
		font-size: 12px;
		line-height: 15.6px;
		font-weight: 400;
		color: #fff;
		white-space: nowrap;
		max-width: 80px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	@media (max-width: 350px) {
		.rule-right {
			gap: 6px;
		}
		.server-pill-name {
			max-width: 58px;
		}
	}
</style>
