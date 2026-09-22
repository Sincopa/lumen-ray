# Выпуск Lumen через GitHub Releases

Автообновление использует подписанные артефакты Tauri. Закрытый ключ нельзя
добавлять в Git, а потеря ключа лишит уже установленные копии возможности
обновляться на новые версии.

## Однократная настройка

1. Локально сгенерируйте ключ обновлений:

   ```powershell
   New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.tauri" | Out-Null
   node node_modules\@tauri-apps\cli\tauri.js signer generate --ci -w "$env:USERPROFILE\.tauri\lumen-updater.key"
   ```

2. Откройте репозиторий GitHub → **Settings → Secrets and variables → Actions**.
3. На вкладке **Secrets** создайте `TAURI_SIGNING_PRIVATE_KEY`. Значение — всё
   содержимое файла `%USERPROFILE%\.tauri\lumen-updater.key`.
4. Если ключ создавался с паролем, создайте секрет
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Для ключа без пароля он не нужен.
5. На вкладке **Variables** создайте `TAURI_UPDATER_PUBLIC_KEY`. Значение — всё
   содержимое файла `%USERPROFILE%\.tauri\lumen-updater.key.pub`.
6. Сохраните закрытый ключ и пароль в резервной защищённой копии. Публичный ключ
   можно хранить открыто, закрытый — нельзя.

Workflow уже имеет `contents: write`. Если политика организации запрещает запись,
в **Settings → Actions → General → Workflow permissions** разрешите GitHub Actions
создавать релизы.

## Новый релиз

```powershell
npm run release:version -- 0.2.0
npm run check
npm test
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "release: v0.2.0"
git tag v0.2.0
git push origin main
git push origin v0.2.0
```

Тег запускает `.github/workflows/release.yml`. Workflow проверяет проект, собирает
NSIS-инсталлятор и подписанный updater-архив, создаёт публичный GitHub Release и
прикладывает `latest.json`. Клиенты проверяют этот файл при запуске; при наличии
версии новее показывается кнопка с колокольчиком.

Версия тега должна совпадать с версией приложения. Скрипт `release:version`
синхронно обновляет `package.json`, `package-lock.json`, `Cargo.toml`,
`Cargo.lock` и `tauri.conf.json`.
