<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import Dialog from '../../components/Dialog.svelte';
	import Dropdown from '../../components/Dropdown.svelte';
	import SettingRow from '../../components/SettingRow.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import { i18n } from '../../lib/i18n';
	import { runtime } from '../../lib/runtime';
	import { catalog } from '../../lib/services';
	import {
		nativeSettings,
		updateNativeSettings,
		type NativeSettings,
	} from '../../lib/services/settings';
	import { importPreview } from '../../lib/services/importPreview';
	let {
		onClose,
		initialImport = '',
	}: { onClose: () => void; initialImport?: string } = $props();
	const label = (ru: string, en: string) =>
		i18n.currentLanguage === 'ru' ? ru : en;
	let section = $state('dns');
	let policy = $state(
		structuredClone($state.snapshot($nativeSettings.dnsPolicy)),
	);
	let fallbacks = $state(policy.fallback.join('\n'));
	let busy = $state(false),
		message = $state(''),
		failed = $state(false);
	let password = $state(''),
		backup = $state('');
	let backupPreview = $state<{
		servers: number;
		subscriptions: number;
		rules: number;
	} | null>(null);
	let input = $state(''),
		preview = $derived.by(() => {
			try {
				return importPreview(input);
			} catch {
				return null;
			}
		});
	$effect(() => {
		if (initialImport) {
			input = initialImport;
			section = 'import';
		}
	});
	let host = $state('');
	let result = $state('');
	const catalogStore = catalog.state;
	async function inspect() {
		const info = await invoke<{
			host: string;
			resolvedAddresses: number;
			systemHttpStatus: number | null;
			proxyHttpStatus: number | null;
			route: string;
			requiresCoreEvaluation: boolean;
		}>('inspect_destination', { input: host });
		const route =
			$catalogStore.servers.find((s) => s.id === info.route)?.name ??
			$catalogStore.subscriptions.find((s) => s.id === info.route)?.name ??
			(info.route === 'direct'
				? label('Напрямую', 'Direct')
				: label('Через VPN', 'VPN'));
		result = [
			info.host,
			`${label('Найдено DNS-адресов', 'DNS addresses')}: ${info.resolvedAddresses}`,
			`${label('Системное соединение', 'System connection')}: ${info.systemHttpStatus ?? '—'}`,
			`${label('Локальный прокси', 'Local proxy')}: ${info.proxyHttpStatus ?? '—'}`,
			`${label('Предварительный маршрут', 'Estimated route')}: ${route}`,
			info.requiresCoreEvaluation
				? label(
						'Для точного маршрута требуется проверка ядром.',
						'Core evaluation is required for the exact route.',
					)
				: '',
		]
			.filter(Boolean)
			.join('\n');
	}
	async function history() {
		const rows = await invoke<
			{
				subscriptionId: string;
				at: number;
				added: number;
				updated: number;
				removed: number;
			}[]
		>('get_maintenance_history');
		result =
			rows
				.slice()
				.reverse()
				.map(
					(row) =>
						`${new Date(row.at).toLocaleString(i18n.currentLanguage)} · ${$catalogStore.subscriptions.find((s) => s.id === row.subscriptionId)?.name ?? label('Подписка', 'Subscription')}\n${label('Добавлено', 'Added')}: ${row.added} · ${label('Изменено', 'Updated')}: ${row.updated} · ${label('Удалено', 'Removed')}: ${row.removed}`,
				)
				.join('\n\n') || label('История пока пуста.', 'No updates yet.');
	}
	async function connections() {
		const rows = await invoke<
			{ process: string; address: string; port: number }[]
		>('get_active_connections');
		result =
			rows
				.map((row) => `${row.process || '—'} → ${row.address}:${row.port}`)
				.join('\n') ||
			label('Нет активных TCP-соединений.', 'No active TCP connections.');
	}
	async function run(action: () => Promise<void>) {
		if (busy || !runtime.isTauri) return;
		busy = true;
		message = '';
		failed = false;
		try {
			await action();
			message = label('Готово', 'Done');
		} catch (error) {
			failed = true;
			const code =
				typeof error === 'object' &&
				error &&
				'code' in error &&
				typeof error.code === 'string' &&
				/^[A-Z_]{1,64}$/.test(error.code)
					? error.code
					: '';
			message =
				label(
					'Операция не выполнена. Проверьте данные и подключение.',
					'Operation failed. Check the input and connection.',
				) + (code ? ` (${code})` : '');
		} finally {
			busy = false;
		}
	}
	function download(name: string, content: string) {
		const url = URL.createObjectURL(
			new Blob([content], { type: 'application/octet-stream' }),
		);
		const a = document.createElement('a');
		a.href = url;
		a.download = name;
		a.click();
		setTimeout(() => URL.revokeObjectURL(url), 10000);
	}
	async function readBackup(event: Event) {
		backup = '';
		backupPreview = null;
		const file = (event.target as HTMLInputElement).files?.[0];
		if (!file) return;
		await run(async () => {
			if (file.size > 24 * 1024 * 1024) throw new Error();
			backup = await file.text();
		});
	}
	async function readQr(event: Event) {
		const file = (event.target as HTMLInputElement).files?.[0];
		if (!file) return;
		await run(async () => {
			if (file.size > 10 * 1024 * 1024) throw new Error();
			const bitmap = await createImageBitmap(file);
			try {
				const scale = Math.min(1, 2048 / Math.max(bitmap.width, bitmap.height));
				const canvas = document.createElement('canvas');
				canvas.width = Math.round(bitmap.width * scale);
				canvas.height = Math.round(bitmap.height * scale);
				const ctx = canvas.getContext('2d', { willReadFrequently: true });
				if (!ctx) throw new Error();
				ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
				const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height);
				const { default: jsQR } = await import('jsqr');
				const qr = jsQR(pixels.data, pixels.width, pixels.height);
				if (!qr) throw new Error();
				input = importPreview(qr.data).input;
			} finally {
				bitmap.close();
			}
		});
	}
</script>

<Dialog
	title={label('Инструменты', 'Tools')}
	onClose={() => {
		if (!busy) onClose();
	}}
>
	<div class="tools">
		<Dropdown
			label={label('Раздел', 'Section')}
			value={section}
			disabled={busy}
			onChange={(v) => {
				section = v;
				message = '';
				result = '';
			}}
			options={[
				{ value: 'dns', label: 'DNS' },
				{ value: 'subscriptions', label: label('Подписки', 'Subscriptions') },
				{ value: 'backup', label: label('Резервная копия', 'Backup') },
				{ value: 'import', label: label('Импорт / QR', 'Import / QR') },
				{ value: 'diagnostics', label: label('Диагностика', 'Diagnostics') },
				{
					value: 'history',
					label: label('История обновлений', 'Update history'),
				},
			]}
		/>
		{#if !runtime.isTauri}<p>
				{label(
					'Доступно в приложении Lumen для Windows.',
					'Available in the Lumen Windows application.',
				)}
			</p>{/if}
		<fieldset disabled={busy || !runtime.isTauri}>
			{#if section === 'dns'}
				<SettingRow label={label('Раздельный DNS', 'Split DNS')}>
					<ToggleSwitch ariaLabel="DNS" bind:checked={policy.enabled} />
				</SettingRow>
				<p>
					{label(
						'DNS следует доменным правилам. Резервные серверы используются внутри выбранного маршрута. Системный DNS остаётся доступным для прямых доменов.',
						'DNS follows domain rules. Fallback servers stay within the selected route. System DNS remains available for direct domains.',
					)}
				</p>
				<label>
					{label(
						'Прямые домены: IP, HTTPS или system',
						'Direct domains: IP, HTTPS or system',
					)}
					<input bind:value={policy.direct} spellcheck={false} />
				</label>
				<label>
					{label(
						'Проксируемые домены: IP или HTTPS',
						'Proxied domains: IP or HTTPS',
					)}
					<input bind:value={policy.proxy} spellcheck={false} />
				</label>
				<label>
					{label('Bootstrap DNS (IP)', 'Bootstrap DNS (IP)')}
					<input bind:value={policy.bootstrap} spellcheck={false} />
				</label>
				<label>
					{label(
						'Резервный DNS прокси, до 3, по одному в строке',
						'Proxy fallback DNS, up to 3, one per line',
					)}
					<textarea
						bind:value={fallbacks}
						rows="3"
						spellcheck={false}></textarea>
				</label>
				<Dropdown
					label="IPv6"
					value={policy.ipv6}
					onChange={(v) => {
						policy.ipv6 = v as 'auto' | 'ipv4';
					}}
					options={[
						{ value: 'auto', label: label('IPv4 + IPv6', 'IPv4 + IPv6') },
						{
							value: 'ipv4',
							label: label('Только IPv4 в туннеле', 'IPv4 only in tunnel'),
						},
					]}
				/>
				<button
					onclick={() =>
						run(() =>
							updateNativeSettings({
								dnsPolicy: {
									...policy,
									fallback: fallbacks
										.split(/\r?\n/)
										.map((v) => v.trim())
										.filter(Boolean),
								},
							}),
						)}
				>
					{label('Применить DNS', 'Apply DNS')}
				</button>
			{:else if section === 'subscriptions'}
				<SettingRow
					label={label('Загрузка подписок', 'Subscription downloads')}
				>
					<Dropdown
						label={label('Маршрут загрузки', 'Download route')}
						value={$nativeSettings.subscriptionTransport}
						onChange={(v) =>
							run(() =>
								updateNativeSettings({
									subscriptionTransport:
										v as NativeSettings['subscriptionTransport'],
								}),
							)}
						options={[
							{
								value: 'direct',
								label: label('Системное соединение', 'System connection'),
							},
							{
								value: 'proxy',
								label: label('Через подключённый прокси', 'Connected proxy'),
							},
							{
								value: 'auto',
								label: label('Системное → прокси', 'System → proxy'),
							},
						]}
					/>
				</SettingRow>
				<p>
					{label(
						'При включённом TUN системное соединение следует его маршрутам. Автоматический режим повторяет сетевые ошибки через локальный прокси; отказ провайдера не повторяется.',
						'With TUN enabled, system connections follow its routes. Automatic mode retries network failures through the local proxy; provider rejections are not retried.',
					)}
				</p>
			{:else if section === 'backup'}
				<p>
					{label(
						'Копия содержит профили, правила и настройки. Шифрование паролем; без него восстановление невозможно. Импорт заменит текущие данные и отключит VPN.',
						'The backup contains profiles, rules and settings. It is password encrypted and cannot be recovered without the password. Restore replaces current data and disconnects the VPN.',
					)}
				</p>
				<label>
					{label(
						'Пароль — не менее 12 символов',
						'Password — at least 12 characters',
					)}
					<input
						type="password"
						autocomplete="new-password"
						bind:value={password}
						oninput={() => {
							backupPreview = null;
						}}
					/>
				</label>
				<button
					disabled={password.length < 12}
					onclick={() =>
						run(async () =>
							download(
								'lumen-backup.lumen',
								await invoke<string>('export_backup', { password }),
							),
						)}
				>
					{label('Сохранить зашифрованную копию', 'Save encrypted backup')}
				</button>
				<label>
					{label('Файл копии', 'Backup file')}
					<input type="file" accept=".lumen" onchange={readBackup} />
				</label>
				<button
					disabled={!backup || password.length < 12}
					onclick={() =>
						run(async () => {
							backupPreview = await invoke('preview_backup', {
								input: backup,
								password,
							});
						})}
				>
					{label('Проверить копию', 'Preview backup')}
				</button>
				{#if backupPreview}
					<p>
						{label(
							'Подписки / серверы / правила',
							'Subscriptions / servers / rules',
						)}: {backupPreview.subscriptions} / {backupPreview.servers} / {backupPreview.rules}
					</p>
					<button
						onclick={() =>
							run(async () => {
								await invoke('restore_backup', { input: backup, password });
								nativeSettings.set(await invoke('get_settings'));
								password = '';
								backup = '';
								backupPreview = null;
							})}
					>
						{label(
							'Заменить данные и восстановить',
							'Replace data and restore',
						)}
					</button>
				{/if}
			{:else if section === 'import'}
				<label>
					{label(
						'Ссылка на подписку или сервер',
						'Subscription or server link',
					)}
					<textarea rows="3" bind:value={input} spellcheck={false}></textarea>
				</label>
				<label>
					{label('QR из изображения', 'QR from image')}
					<input
						type="file"
						accept="image/png,image/jpeg,image/webp"
						onchange={readQr}
					/>
				</label>
				{#if preview}<p>
						{label('Источник', 'Source')}: {preview.description}
					</p>{/if}
				<p>
					{label(
						'Добавление начнётся только после нажатия кнопки. Ссылка от внешнего приложения не подключает VPN автоматически.',
						'Import starts only after pressing the button. Links from external applications do not connect the VPN automatically.',
					)}
				</p>
				<button
					disabled={!preview}
					onclick={() =>
						run(async () => {
							if (preview) await catalog.importSubscription(preview.input);
							input = '';
						})}
				>
					{label('Добавить профиль', 'Add profile')}
				</button>
			{:else if section === 'diagnostics'}
				<label>
					{label('Домен или HTTPS-адрес', 'Domain or HTTPS address')}
					<input
						bind:value={host}
						placeholder="example.com"
						spellcheck={false}
					/>
				</label>
				<button disabled={!host.trim()} onclick={() => run(inspect)}>
					{label(
						'Проверить доступность и правила',
						'Check reachability and rules',
					)}
				</button>
				<p>
					{label(
						'Правила — предварительная оценка. GeoIP, GeoSite и правила приложений требуют обработки ядром. Системные TCP-соединения не показывают фактический маршрут Xray.',
						'Rules are a preliminary estimate. GeoIP, GeoSite and application rules require core evaluation. System TCP connections do not show the actual Xray route.',
					)}
				</p>
				<button onclick={() => run(connections)}>
					{label('Активные TCP-соединения', 'Active TCP connections')}
				</button>
				<button
					onclick={() =>
						run(async () =>
							download(
								'lumen-diagnostics.json',
								JSON.stringify(
									await invoke('export_diagnostic_report'),
									null,
									2,
								),
							),
						)}
				>
					{label(
						'Сохранить отчёт без адресов и ключей',
						'Save report without addresses or keys',
					)}
				</button>
			{:else if section === 'history'}
				<p>
					{label(
						'Последние 100 обновлений подписок с количеством добавленных, изменённых и удалённых серверов.',
						'Last 100 subscription updates with counts of added, updated and removed servers.',
					)}
				</p>
				<button onclick={() => run(history)}>
					{label('Загрузить историю', 'Load history')}
				</button>
			{/if}
		</fieldset>
		{#if busy}<p role="status">{label('Выполняется…', 'Working…')}</p>{/if}
		{#if message}<p role={failed ? 'alert' : 'status'}>{message}</p>{/if}
		{#if result}<pre>{result}</pre>{/if}
	</div>
</Dialog>

<style>
	.tools {
		display: flex;
		flex-direction: column;
		gap: 14px;
		overflow-y: auto;
		max-height: calc(100dvh - 180px);
		padding: 4px 2px 12px;
	}
	fieldset {
		border: 0;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 14px;
		min-width: 0;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 8px;
		font-size: 13px;
	}
	input,
	textarea,
	button {
		font: inherit;
		color: inherit;
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 10px;
		background: rgba(255, 255, 255, 0.05);
		padding: 10px;
		min-width: 0;
	}
	textarea {
		resize: vertical;
	}
	button {
		cursor: pointer;
	}
	button:disabled,
	fieldset:disabled {
		opacity: 0.5;
	}
	p {
		font-size: 12px;
		line-height: 1.5;
		opacity: 0.7;
		margin: 0;
	}
	pre {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		font-size: 11px;
		max-height: 220px;
		overflow: auto;
	}
	:focus-visible {
		outline: 2px solid var(--text-primary);
		outline-offset: 2px;
	}
</style>
