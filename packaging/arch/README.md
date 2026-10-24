# osu! radio для Arch Linux x86_64

Источник — опубликованный тег `v0.1.0-alpha`, commit
`bf941c29b2e69fa73aca6c1c6a7c1643f3a7f3f9`. В [PKGBUILD](PKGBUILD) и
[.SRCINFO](.SRCINFO) записан SHA-256 точного архива GitHub:
`15fa8e08b960cf00a47ec1ae935bf90c774bfccc17dc82f1100b0bf3668f1566`.
Не заменяйте checksum на `SKIP` и не подменяйте тег веткой.
Публикация в AUR не выполнялась; пользовательская приёмка KDE/импорта/звука
остаётся отдельной проверкой T06.

## Сборка и установка

Нужны актуальный Arch Linux x86_64, графическая сессия и `base-devel`.
Собирайте от обычного пользователя в отдельном рабочем каталоге. Скопируйте
туда `PKGBUILD` и `osu-radio.desktop` из завершённого комплекта упаковки.
Первичная сборка требует интернета: GitHub archive, закреплённый Rust toolchain,
Cargo crates и NuGet packages загружаются автоматически. Предусмотрите место
для SDK, кэшей и release-сборки; личные кэши и готовые бинарники не нужны.

Зависимости сборки: `rustup`, `dotnet-sdk-8.0`, `lld`, `pkgconf`, `clang`.
Arch `rustup` конфликтует с пакетом `rust` и предоставляет Cargo/Rust proxies.
Если установлен `rust`, перед сборкой выберите переход на `rustup` через pacman;
не используйте принудительное игнорирование конфликтов. Версию Rust рецепт
берёт из `rust-toolchain.toml`, а SDK выбирает из установленных .NET 8 SDK.
Системный `HOME` рецепт не меняет.

## Подготовка текущего checkout к следующей публикации

Source pin выше остаётся прежним: опубликованный архив `v0.1.0-alpha`
не содержит новых регулируемых скорости/pitch. Обновлённые prerequisites
и `licenses/` относятся к текущему checkout; они не изменяют содержимое
старого архива и установленных пакетов. Перед следующим релизом нужно
повторно сверить notices с окончательным lockfile и helper publish.

`signalsmith-stretch = 0.1.3` компилирует встроенные Signalsmith Stretch
и Linear через C++14, а bindgen загружает libclang при сборке. C++ compiler
даёт `base-devel`; Arch `clang` предоставляет libclang и resource headers.
Отдельные runtime-зависимости на clang или Signalsmith не нужны:
DSP статически встроен, C++ runtime уже объявлен в `depends`.
В обычной системной установке `LIBCLANG_PATH` не требуется; для нестандартного
LLVM см. [требования и диагностику](../../docs/agent/development.md#signalsmith-native-build).

Для проверки нового кода подготовьте отдельную копию checkout без `.git`,
`target`, `packaging/arch/src`, `packaging/arch/pkg` и старых пакетов/архивов.
В отдельной временной копии рецепта замените только source archive на архив
этой копии с верхним каталогом `osu-radio-0.1.0-alpha` и вычислите его SHA-256.
Выполните `makepkg` без `-i`. Это local-source проверка build/package функций,
а не проверка опубликованного архива; не переносите временные source/checksum
в release recipe. Установка и публикация относятся к отдельной приёмке.

## Runtime и запуск

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
Это проверки материалов. T05 также проверила clean Arch chroot, состав,
зависимости и установленные Qt/server/настоящий helper в отдельной среде
без исходников и SDK. Native Realm 20.1.0 поставляется из NuGet с partial RELRO;
это известное ограничение hardening prebuilt library, не исправляемое LDFLAGS
настоящего рецепта. KDE, импорт пользовательских stable/lazer библиотек,
физический звук и сохранность данных при обновлении ещё требуют T06.

Список зависимостей сверён 2026-10-17 с [Arch package metadata](https://archlinux.org/packages/)
и [AUR RPC](https://aur.archlinux.org/rpc/v5/info?arg[]=osu&arg[]=osu-lazer-bin).
Формат рецепта: [PKGBUILD(5)](https://man.archlinux.org/man/PKGBUILD.5.en).
