# BUGSAUDIT-2026-10-13

## 1. Шапка

- Дата: 2026-10-13
- Ревизия: `c52cd74ca3bdda032dd71df20e214674bb32956b` (`fix: route Russian sites direct for VPN-bound app rules`)
- Рабочее дерево на старте было грязным. Чужие незакоммиченные правки не откатывались и не коммитились целиком. Список в разделе 6.
- Проверял чтением исходников (не вендор, не `target`, не `node_modules`), `git log -40 --stat`, `git diff` незакоммиченных файлов, точечными тестами.
- Прошлых `BUGSAUDIT-*` в репозитории нет. `TODO`/`FIXME` в продуктовом коде нет; `unwrap` в основном в тестах.
- Граф кода проиндексирован 2026-10-02 и отстаёт от текущих коммитов, поэтому утверждения сверены по файлам, а не по графу.

## 2. Покрытие

| Область | Статус | Чем смотрел | Почему не целиком |
| --- | --- | --- | --- |
| `src-tauri/src/platform/helper.rs` сессия TUN, stop/reload/respawn | просмотрено | чтение `run_session`, `release_then_stop`, `apply_reload`, `respawn_standby`, `Helper::activate` | — |
| `src-tauri/src/platform/network.rs` маршруты | просмотрено | `release_lumen_routes`, `routes_released`, `TunnelRoutes::activate/deactivate`, `tunnel_route_exists` | — |
| `src-tauri/src/core/xray.rs` connect/stop/live routing | просмотрено | `stop`, `connect`, `apply_live_rules`, `run_live_outbound_add` | не каждую ветку `switch_server_live` построчно |
| `src-tauri/src/services/vpn_service.rs`, `vpn_monitor.rs` | просмотрено | connect/disconnect/release, монитор, recovery | — |
| `src-tauri/src/xray/dns_policy.rs`, `xray/config.rs` routing | просмотрено | diff и `build` | — |
| `src-tauri/src/services/catalog_service.rs` пресеты и порядок правил | просмотрено | `generate_preset_rules`, `ordered_rules`, `ApplyPreset` | импорт подписки не построчно |
| `src/lib/services/catalog.ts`, `TauriCatalogService.ts`, `PresetsModal.svelte` | просмотрено | diff пресетов и `orderedRules` | экраны правил кроме пресетов не построчно |
| `src/lib/stores/connection.ts`, `errors.ts` | просмотрено | reconcile, таймаут, карта кодов | — |
| `src-tauri/src/lib.rs` выход из приложения | просмотрено | `ExitRequested` | трей и меню не построчно |
| `src-tauri/src/subscription/fetcher.rs` | частично | редиректы и лимит 5 | разбор тел подписок не построчно |
| `src-tauri/src/core/lumen/*` | частично | `update_routing`, начало `RoutingTable::compile` | proxy/dns побайтно не читал |
| `src-tauri/src/storage/secure.rs`, `repository.rs` | частично | DPAPI и атомарная запись | миграции полей не сверял с каждой версией |
| `src-tauri/src/platform/process.rs` | частично | spawn, job, kill | — |
| UI: Rules/Settings/Subscriptions/Servers | частично | поиск пустых `catch` | вёрстку и каждый обработчик не открывал |
| e2e, lottie, скрипты релиза | не смотрел | — | не на пути подключения и правил |
| `src-tauri/src/xray/updater.rs` проверка подписи | не смотрел | — | отдельный контур обновления, в этот проход не входил |

Где по просмотренному пути дефекта нет:

- Семафор `connection_operation` держится на время `mem::take` сессии в мониторе и в `connect_with_selection`, поэтому параллельный connect не затирает вынутую сессию.
- Повторный `api ado` с текстом `existing tag` считается успехом, обрыв между добавлением outbound и патчем правил не застревает навсегда.
- Пресеты `russia-optimized`, `bypass-blocking`, `russia-direct` в Rust, `mergePreset` и `PresetsModal` сейчас содержат одни и те же geo-теги, включая telegram. Расхождение содержимого на эту ревизию не нашёл.
- `release_then_stop` и цикл `retire` после закрытия pipe специально не убивают Xray, пока маршрут не снят. Это не зависание ради зависания: job object при выходе процесса убил бы Xray и оставил бы маршрут чёрной дырой. См. журнал решений.

## 3. Находки

### B1 · `XraySession::stop` · `src-tauri/src/core/xray.rs` · P1 · подтверждено · исправлено в рабочем дереве

Неудачный stop стирал подготовленное состояние и `started`, хотя helper или дочерний процесс оставались живы.

Как заметить: disconnect/stop, при котором helper отвечает не `1` (маршруты ещё стоят) или `TerminateProcess` не срабатывает. В снимке сессии `started == None`, `prepared_*` пустые, процесс при этом ещё читает адаптер.

Последствие: монитор смотрит только на `started()` и перестаёт следить за живым туннелем. Следующее подключение не знает, какой конфиг реально крутится.

Почему это баг: комментарий в том же коде говорит «оставить pipe, потому что маршруты ещё стоят». Стирание учёта этому противоречит. Успешный stop по-прежнему всё очищает.

Было: после `session.helper = Some(helper)` поля `prepared_*` и `started` очищались безусловно. Стало: очистка только если ни helper, ни child не возвращены в сессию (`retained`).

### B2 · `tunnel_route_exists` / `defaults_absent_from_reads` · `src-tauri/src/platform/network.rs` · P1 · подтверждено · исправлено в рабочем дереве

Ошибка `GetIpForwardTable2` читалась как «маршрута нет».

Как заметить: `routes_released` и `tunnel_defaults_absent` зовутся из `release_then_stop`, `release_to_standby` и `standby_isolated`. Если таблицу не прочитали, старый код возвращал `false` из `tunnel_route_exists`, и оба семейства выглядели чистыми. При метрике адаптера `>= 5000` `routes_released` становился `true`, и helper убивал Xray.

Последствие: маршрут по умолчанию мог остаться без процесса, который его обслуживает.

Почему это баг: `delete_lumen_default_routes` на ту же ошибку возвращает `Err`. Проверка «уже снято» была открыта в другую сторону. Пропавший адаптер по-прежнему считается чистым (`tunnel_luid() == None` → `Some(false)`).

Исправление: чтение возвращает `Option<bool>`, `None` не считается отсутствием маршрута. Тест `unreadable_route_table_is_not_treated_as_clear`.

### B3 · `respawn_standby` · `src-tauri/src/platform/helper.rs` · P1 · подтверждено · исправлено в рабочем дереве

Падение Xray в standby перезапускало процесс, писало в лог успех и оставляло неготовый процесс, даже если `wait_for_standby` не дождался порта.

Как заметить: Xray в elevated helper завершился, маршруты снялись, новый процесс не открыл SOCKS. Sweep крутится только когда `child == None`, поэтому повторный запуск не начинался, пока этот процесс сам не умрёт.

Последствие: подготовленный туннель молча не восстанавливается.

Исправление: при неудачном ожидании процесс убивается, `child = None`, успех в лог пишется только после готовности. Sweep может запустить цикл снова.

### B4 · `apply_reload` · `src-tauri/src/platform/helper.rs` · P1 · подтверждено · исправлено в рабочем дереве

Новые `bytes` / `core` / `proxy_port` записывались до `wait_for_standby`. Если ожидание или проверка изоляции падали, в сессии оставался неготовый процесс, а Sweep перезапустил бы уже сломанный конфиг.

Как заметить: reload helper (смена ядра или конфига без нового UAC) при таймауте адаптера.

Исправление: поля сессии обновляются только после готовности и `standby_isolated`. Иначе процесс убивается, `child = None`, предыдущие bytes остаются для Sweep.

### B5 · `ExitRequested` · `src-tauri/src/lib.rs` · P1 · подтверждено · исправлено в рабочем дереве

Выход делал `session.try_lock()`. Если монитор держал сессию (health или `release_to_standby`), stop пропускался. Disconnect перед этим для TUN только снимает маршруты и оставляет elevated helper.

Как заметить: закрыть приложение в ту же секунду, когда монитор держит `session`. В диспетчере остаётся второй процесс приложения (`--lumen-tunnel`).

Исправление: `lock().await` внутри уже существующего бюджета 25 секунд. Disconnect к этому моменту лок отпустил, взаимной блокировки с его захватом нет.

### B6 · `ConnectionController.reconcileAfterError` · `src/lib/stores/connection.ts` · P1 · подтверждено · исправлено

Таймаут IPC записывался в `error`, даже если снимок к этому моменту уже `connected`.

Как заметить: TUN дольше клиентского таймаута (в Tauri это 120 с, вместе с UAC и helper). `withTimeout` не отменяет `invoke`. Снимок уже `connected`, на экране ещё ошибка «время ожидания истекло».

Почему это баг: тест на статус `connecting` специально оставляет `error: timeout` и `busy: true`, чтобы не было второго клика. Для уже установленной сессии эта ошибка ложная. Ветка `connecting` не менялась.

Исправление: при `status === 'connected'` показывается только ошибка снимка. Тест `drops a client timeout once the native session is already connected`.

### B7 · `Helper` command `1` · `src-tauri/src/platform/helper.rs` · P2 · подтверждено · исправлено в рабочем дереве

`TunnelRoutes::activate` при `TUN_FAILED` отвечал байтом `2`. Клиент `Helper::activate` читает `2` как `TUN_ADAPTER_TIMEOUT`.

Как заметить: не создался маршрут или интерфейс не готов — пользователь видит «Windows не подготовил адаптер», хотя это другой сбой.

Исправление: такой ответ теперь `5`. Клиентский `_` уже мапит его в общий `TUN_FAILED` и не ждёт лишний кадр, рассинхрона pipe нет.

### B8 · `errorCode` · `src/lib/services/errors.ts` · P2 · подтверждено · исправлено

Коды `LIVE_SWITCH_FAILED`, `IP_UNAVAILABLE`, `INVALID_CONFIG`, `DNS_UNAVAILABLE`, `BACKUP_FAILED`, `BACKUP_INVALID`, `ALREADY_RUNNING`, `UPDATE_FAILED`, `UPDATE_UNAVAILABLE`, `GEO_UPDATE_FAILED`, `NETWORK_ERROR`, `CORE_STOP_FAILED`, `METRICS_UNAVAILABLE` не были в карте и показывались как `unknown`.

Исправление: привязаны к уже существующим текстам `unavailable`, `storage`, `geoDataMissing`. Тест в `tests/unit/coreEngine.test.ts`.

### B9 · пресеты правил в трёх местах · P2 · подтверждено · не исправлено

Один и тот же список живёт в `catalog_service::generate_preset_rules`, `mergePreset` (`catalog.ts`) и тегах `PresetsModal.svelte`. Сейчас telegram добавлен во все три. Следующая правка одного файла снова разведёт UI, оптимистичный кэш и то, что сохранится.

Почему не чинил: это три рантайма (Rust, оптимистичный клиент, подписи модалки). Вынос в один генератор шире бага и заденет незакоммиченный diff пресетов. На этой ревизии содержимое совпадает, поэтому ложного маршрута из-за расхождения нет.

### B10 · `release_then_stop` · `src-tauri/src/platform/helper.rs` · P1 · подтверждено · не исправлено

Цикл без предела: пока `routes_released()` ложен, helper не отвечает родителю и не читает pipe. `start_internal` ждёт байт 120 с и при таймауте роняет `ProcessHandle` через `CloseHandle`, не завершая elevated-процесс.

Как заметить: старт TUN, Xray умер до готовности, маршрут не удаляется. Родитель показывает ошибку, процесс `--lumen-tunnel` остаётся.

Почему не оборвал цикл таймаутом: выход helper закрывает job с `KILL_ON_JOB_CLOSE` и убивает Xray. Если маршрут ещё стоит, система остаётся без пути по умолчанию. Бесконечный цикл здесь удерживает процесс живым. Чинить надо удержанием handle у родителя и повторной попыткой stop, а не убийством. Это отдельное изменение протокола, его не делал.

## 4. Журнал решений

- DNS больше не повторяет пользовательские direct-правила: `dns_policy::build` всегда отдаёт direct только `local`/`lan`/`localhost`/`geosite:category-ru`, остальное идёт в proxy-резолвер. Тест `server_list_ignores_routing_mode_and_user_rules` и комментарий фиксируют это, чтобы список DNS не менялся и живое обновление правил не требовало перезапуска. Откатить это значит снова получать `KeepTunnel` и откат каталога на каждой правке правила. Не трогал. Прямой домен вне `category-ru` резолвится через proxy-DNS — это цена текущего контракта, не случайный пропуск.
- `should_release_tunnel` игнорирует `physical_changed`. Тест прямо требует: смена сети сама по себе туннель не снимает, снимают три неудачных проверки. Не баг.
- Первый `mergePreset` в `TauriCatalogService.applyPreset` выглядит лишним, но бросает `invalidRule` до `pending.push`. Если убрать его, сломанный пресет попадёт в очередь оптимистичных правок и сломает `render`. Оставил.
- `std::thread::sleep` до 1.5 с внутри `TunnelRoutes::activate` блокирует runtime helper. В этой сессии других задач нет, родитель как раз ждёт ответ. На протокол не влияет. Не переписывал в async.
- Ложный `TUN_ADAPTER_TIMEOUT` заменён на общий `TUN_FAILED`, а не на новый код в локалях: отдельного текста для «не смог включить маршрут» в UI нет, врать про таймаут адаптера хуже.

## 5. Журнал правок

Rust-файлы уже были грязными до аудита. Коммит всего файла забрал бы чужой diff, интерактивный `git add -p` недоступен. Поэтому B1–B5 и B7 лежат в рабочем дереве и не закоммичены.

Проверки:

- `cargo test --lib unreadable_route_table_is_not_treated_as_clear` — до правки теста не было; после: `ok. 1 passed`. Сборка lib при этом прошла, значит `stop`, `helper` и `lib.rs` компилируются.
- `npx vitest run tests/unit/connection.test.ts tests/unit/coreEngine.test.ts` — `2 passed`, `18 passed`. Новый тест на B6 падал бы на старом `reconcileAfterError` (ошибка `timeout` при статусе `connected`).

Отдельного теста на B1/B3/B4/B5 нет: это ветки процесса и Win32, без helper их не поднять. Политика «нечитаемая таблица ≠ маршрут снят» покрыта тестом B2.

## 6. Не сделано

- B9, три копии пресетов. Причина в разделе 3.
- B10, удержание elevated-процесса после таймаута старта. Причина: обрыв цикла убивает Xray при живом маршруте.
- Полный `cargo test` и e2e не гонял. Прогнаны точечные тесты выше; lib собрался.
- Незакоммиченные чужие файлы на старте, их не откатывал: `.claude/launch.json` (удалён), `commands/catalog.rs`, `core/lumen/mod.rs`, `core/mod.rs`, `core/xray.rs`, `lib.rs`, `platform/helper.rs`, `platform/network.rs`, `platform/smoke.rs`, `services/catalog_service.rs`, `services/vpn_monitor.rs`, `services/vpn_service.rs`, `tests.rs`, `xray/config.rs`, `xray/dns_policy.rs`, `PresetsModal.svelte`, `TauriCatalogService.ts`, `catalog.ts`, `tests/unit/catalog.test.ts`.
- В части этих файлов теперь есть и правки аудита: `core/xray.rs`, `lib.rs`, `platform/helper.rs`, `platform/network.rs`.

## 7. Подготовлено, но не выполнено

Нет действий вне репозитория. Пуш, миграция данных и завершение чужих процессов не запускались.
