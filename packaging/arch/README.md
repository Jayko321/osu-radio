# osu! radio для Arch Linux x86_64

Комплект **предварительный**: тег `v0.1.0-alpha` ещё не опубликован, первый
SHA-256 в [PKGBUILD](PKGBUILD) — 64 нуля, блокирующие проверку архива.
Финальный checksum и пересоздание [.SRCINFO](.SRCINFO) относятся к T05 после
появления тега. До этого обычная сборка из GitHub не готова. Не заменяйте
checksum на `SKIP` и не подменяйте тег веткой. Публикация в AUR не выполнялась.

## Сборка и установка после завершения T05

Нужны актуальный Arch Linux x86_64, графическая сессия и `base-devel`.
Собирайте от обычного пользователя в отдельном рабочем каталоге. Скопируйте
туда `PKGBUILD` и `osu-radio.desktop` из завершённого комплекта упаковки.
Первичная сборка требует интернета: GitHub archive, закреплённый Rust toolchain,
Cargo crates и NuGet packages загружаются автоматически. Предусмотрите место
для SDK, кэшей и release-сборки; личные кэши и готовые бинарники не нужны.

Зависимости сборки: `rustup`, `dotnet-sdk-8.0`, `lld`, `pkgconf`.
Arch `rustup` конфликтует с пакетом `rust` и предоставляет Cargo/Rust proxies.
Если установлен `rust`, перед сборкой выберите переход на `rustup` через pacman;
не используйте принудительное игнорирование конфликтов. Версию Rust рецепт
берёт из `rust-toolchain.toml`, а SDK выбирает из установленных .NET 8 SDK.
Системный `HOME` рецепт не меняет.

Runtime-зависимости объявлены в PKGBUILD: glibc/GCC runtime, ALSA, Qt 6.8+
(base, declarative, SVG, Wayland), .NET 8 runtime и desktop portal с реализацией.
В KDE установите `xdg-desktop-portal-kde`, предоставляющий
`xdg-desktop-portal-impl`. Для другой среды выберите её реализацию портала.
Портал нужен существующему диалогу выбора папки.

```sh
makepkg -si
```

`-s` предлагает установить зависимости через pacman, `-i` — готовый пакет.
Не запускайте makepkg от root. После установки откройте **osu! radio** в меню
аудиоприложений KDE или выполните `osu-radio`. `.env`, URL базы и отдельный
запуск сервера не нужны. В Settings добавьте папку своей установки osu!.

`osu` (stable) и `osu-lazer-bin` (lazer) указаны только как `optdepends`.
Их не требуется устанавливать вместе; вручную установленная библиотека тоже
поддерживается. Самому osu! radio Wine не нужен. Исходные файлы игры читаются
без копирования аудио и без записи в библиотеку игры.

## Состав и пользовательские данные

- `/usr/bin/osu-radio-qt`, ссылка `/usr/bin/osu-radio` и `/usr/bin/osu-radio-server`;
- `/usr/lib/osu-radio/` — полный framework-dependent publish помощника lazer,
  включая apphost, DLL, runtimeconfig, deps и нативный Realm; встроенного
  .NET runtime нет, используется обязательный `dotnet-runtime-8.0`;
- `/usr/share/applications/osu-radio.desktop` — запуск без терминала, без иконки;
- `/usr/share/licenses/osu-radio/` — MIT проекта и весь [комплект notices](licenses/README.md).

QML и ассеты встроены в приложение. Сервер находится рядом с Qt executable,
путь помощника встроен как `/usr/lib/osu-radio/osu-lazer-realm-parser`.
Исходники и рабочий каталог сборки установленному приложению не нужны.
Для диагностики сохраняется override `OSU_LAZER_REALM_PARSER_PATH`.

SQLite по умолчанию: `$XDG_DATA_HOME/osu-radio/osu-radio.db`; если переменная
не задана, пуста или относительна — `$HOME/.local/share/osu-radio/osu-radio.db`.
Каталог создаётся при первом запуске. Явный `SQLITE_DATABASE_URL` сохраняет
приоритет. Настройки Qt: `$XDG_CONFIG_HOME/osu-radio/osu-radio-qt.conf`
(по умолчанию `$HOME/.config/osu-radio/osu-radio-qt.conf`), с прежними ключами.
Artwork cache находится в памяти; временное аудио использует системный temp
каталог (`TMPDIR`, обычно `/tmp`) и очищается существующим механизмом клиента
после нормального завершения. Нового файлового журнала пакет не создаёт.

Для обновления возьмите новый завершённый комплект, снова выполните `makepkg -si`
и закройте/запустите приложение для использования новых бинарников. Переустановка
готового пакета выполняется обычным `pacman -U путь/к/пакету.pkg.tar.zst`.
Упаковка устанавливает только системные файлы: install hooks и операции сброса
пользовательской БД/настроек отсутствуют. Не удаляйте пользовательские каталоги
при обновлении; обычный запуск не выполняет reset базы.

## Проверки комплекта

```sh
bash -n packaging/arch/PKGBUILD
desktop-file-validate packaging/arch/osu-radio.desktop
makepkg --dir packaging/arch --printsrcinfo
```

Последний вывод должен совпадать с `.SRCINFO`. После изменения source/checksum
пересоздайте его: `makepkg --dir packaging/arch --printsrcinfo > packaging/arch/.SRCINFO`.
Это проверки материалов. Чистый Arch chroot, анализ зависимостей готового пакета,
работа после удаления исходников, KDE, импорт stable/lazer, физический звук
и сохранность данных при обновлении ещё требуют отдельных проверок T05/T06.

Список зависимостей сверён 2026-10-17 с [Arch package metadata](https://archlinux.org/packages/)
и [AUR RPC](https://aur.archlinux.org/rpc/v5/info?arg[]=osu&arg[]=osu-lazer-bin).
Формат рецепта: [PKGBUILD(5)](https://man.archlinux.org/man/PKGBUILD.5.en).
