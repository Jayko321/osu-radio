# Shuffle, repeat и переработка Previous: план реализации

Статус: существенные решения согласованы; карточки T01–T11 подготовлены, T01–T07 выполнены, остальные карточки не начаты.

## Источник и цель

Источник — [спецификация R1–R10](../specs/shuffle-repeat.md). Добавить серверные
режимы shuffle и repeat (Off/All/One), сохраняемые в состоянии очереди, клиентское
правило Previous с порогом 3 секунды, кнопки Qt, моки галереи и управление
режимами через MPRIS/SMTC.

Решения, принятые при обсуждении плана (2026-10-24):

- **Запуск плейлиста с явным стартом при shuffle — как в Spotify.** Выбранный
  пункт играет первым, все остальные пункты плейлиста (в том числе стоящие
  до него) перемешиваются после него. Выключение shuffle возвращает их в порядок
  плейлиста: `[C] → A → B → D`.
- **Изменение режимов — новые команды плеера** в существующем
  `POST /api/playback/commands`: `set_shuffle { enabled }` и
  `set_repeat { mode: "off" | "all" | "one" }`. Новый маршрут не нужен; команды
  идут существующей полосой `PendingPlayback` и упорядочены с Next/Previous.

## Существующий код

Срез исследования: `main`, `e0441f4`.

- [QueueService](../../crates/radio-services/src/queue.rs): все изменения идут
  через `change()` под writer lock; переходы — `apply()` (Next/Previous/Finished/
  Failed), `forward()`, `launch()`, `exhaust()`; место «Играть следующим» —
  `next_index()`; Play из библиотеки — ветка `Change::Command(Play { Some(id) })`;
  `Started` пишет историю через `ListeningHistoryService::record_started`
  при совпадении token и наличии `launch_source`. `assignment()` считает
  `can_next`/`can_previous`.
- Запуск плейлиста: [PlaylistService::play](../../crates/radio-services/src/playlist.rs)
  вычисляет `start` (явный пункт или первый с источником) и вызывает
  `QueueService::replace_in`; добавление плейлиста в конец — `append_playlist_in`.
- [ListeningHistoryRepository::record_started](../../crates/radio-db/src/repositories/listening_history.rs)
  дедуплицирует только по последнему token источника: перезапуск той же записи
  с новым token запишет новую дату. Для R3 нужен отдельный признак.
- [QueueState/QueueEntry](../../crates/radio-db/src/model/queue.rs) с `validate()`,
  [QueueRepository](../../crates/radio-db/src/repositories/queue.rs) (сырой SELECT
  полей `playback_queue` + полная перезапись `queue_entries`), сущности
  [queue](../../crates/radio-db/src/entities/queue.rs) и `queue_entry`, последняя
  миграция — [v15](../../crates/radio-db/src/migrations/m20261020_000015_audio_mods.rs),
  образец аддитивной миграции очереди — [v14](../../crates/radio-db/src/migrations/m20261016_000014_queue_launch_source.rs).
- Сервер: [routes/playback.rs](../../apps/osu-radio-server/src/routes/playback.rs) —
  `CommandRequest`, `PlaybackResponse` (он же строка NDJSON `/api/playback/events`),
  `QueueResponse`; тесты в `playback_tests.rs`, `queue_transport_tests.rs`.
- Клиент: [models.rs](../../crates/osu-radio-client/src/models.rs) —
  `PlaybackAssignment`, `PlaybackCommand`; [controller.rs](../../crates/osu-radio-client/src/controller.rs) —
  `AppCommand::Previous` → `queue_playback(Previous)`;
  [PlayerViewModel](../../crates/osu-radio-client/src/view_models/player.rs) —
  `presentation()`, `system_snapshot()`, `system_event()` (системный Previous тоже
  идёт в `queue_playback`), `seek_selected()`/`system_seek()` через
  `playback::Command::Seek`. Локальный seek в [playback.rs](../../crates/osu-radio-client/src/playback.rs)
  проверяет token и не меняет паузу и не шлёт `Started`.
- Клавиатурной привязки Previous в QML нет: «клавиатура» в R9 — это медиаклавиши
  через MPRIS/SMTC, они идут через `system_event`.
- Qt: [PlayerPanel.qml](../../apps/osu-radio-qt/qml/PlayerPanel.qml) — отключённые
  `shuffleButton`/`repeatButton`; [qt_view_models.rs](../../apps/osu-radio-qt/src/qt_view_models.rs) —
  `PlayerViewModel` (`canPrevious`, `previousTrack()`); иконки в
  `apps/osu-radio-qt/assets/icons/` (Lucide), есть `shuffle.svg`, `repeat-2.svg`,
  нет `repeat-1.svg`. `AdapterProbe.qml:1683` проверяет, что кнопки отключены.
- Галерея не показывает `PlayerPanel`: состояние в [mock.rs](../../crates/osu-radio-client/src/mock.rs)
  (`MockState`, `Action`) и [mock/queue.rs](../../crates/osu-radio-client/src/mock/queue.rs),
  действия из QML разбираются строками в [bridge.rs](../../apps/osu-radio-qt/src/bridge.rs).
- Медиаконтролы: [lib.rs](../../crates/osu-radio-media-controls/src/lib.rs) —
  `Snapshot`, `Capabilities`, `Event`; [linux/player.rs](../../crates/osu-radio-media-controls/src/linux/player.rs)
  — `shuffle`/`loop_status` возвращают unsupported; [windows/mod.rs](../../crates/osu-radio-media-controls/src/windows/mod.rs)
  — SMTC `ButtonPressed`. В `windows 0.62` с уже включённой фичей `Media` есть
  `ShuffleEnabled`/`AutoRepeatMode` и события `*ChangeRequested`.
- `fastrand 2.5.0` уже в `Cargo.lock`.

## Техническое решение

### Данные (radio-db), миграция v16

`m20261024_000016_queue_modes`, аддитивная, в транзакции:

- `playback_queue.shuffle BOOLEAN NOT NULL DEFAULT FALSE`;
- `playback_queue.repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK (repeat_mode IN ('off','all','one'))`;
- `playback_queue.repeat_launch BOOLEAN NOT NULL DEFAULT FALSE` — текущий запуск
  является повтором Repeat One и не записывает прослушивание;
- `queue_entries.shuffle_order BIGINT NULL` — ключ исходного порядка.

Модель: `RepeatMode { Off, All, One }`; `QueueState { shuffle, repeat, repeat_launch }`;
`QueueEntry { shuffle_order: Option<i64> }`. `validate()`: при `shuffle` у всех
записей есть ключ, без `shuffle` — ни у одной. Репозиторий читает/пишет новые поля.
Downgrade по принятой в проекте практике отказывает, если состояние
непредставимо в v15 (shuffle включён или repeat не `off`); иначе удаляет колонки.
Существующие базы получают Off/Off без сброса.

### Сервис очереди (radio-services)

Новые `PlaybackCommand::SetShuffle { enabled }` и `SetRepeat(RepeatMode)` проходят
через `change()` → `apply()`; повторная установка того же значения — no-op без
новой revision. Режимы не трогают current, token и историю.

- **Включение shuffle (R1):** всем записям ключ = текущая позиция; записи
  `next_index()..` перемешиваются Fisher–Yates (`fastrand`, прямая зависимость
  `radio-services`), `reindex()`.
- **Выключение:** записи `next_index()..` стабильно сортируются по ключу, ключи
  очищаются. Перетаскивание (`Move`) ключ не меняет — поэтому возвращается
  в исходное место. Прошлые и текущая не двигаются.
- **Вставки при shuffle (R5):** «Играть следующим» и Play из библиотеки получают
  ключи меньше минимального в очереди, «Добавить в конец» и `append_playlist_in` —
  больше максимального; пачка сохраняет свой внутренний порядок. Так при
  выключении они остаются сразу после текущей / в конце. Без shuffle ключи
  не назначаются.
- **Запуск плейлиста (R5):** `replace_in` получает признак явного старта.
  При shuffle ключи новых записей идут в порядке плейлиста; явный старт ставится
  на границу и запускается, остальные пункты перемешиваются после него;
  без явного старта перемешивается весь плейлист и запускается первая доступная
  запись нового порядка. Без shuffle поведение прежнее.
- **Repeat All (R4, R7):** `forward()` при `All` после поиска от `start` до конца
  продолжает с `0` до `start`; нет доступных — `exhaust()`. Это действует для
  Finished, Next, Failed, удаления текущей и неудачной проверки. После Failed
  поиск не возвращается к упавшей записи (иначе цикл при одной записи).
  Previous не переходит к последней; на первой записи — прежний перезапуск первой.
  `can_next` при `All` — есть ли хоть одна доступная запись.
- **Repeat One (R3, R7):** Finished при `One` и доступной текущей → `launch()`
  той же позиции (новый token — клиент перезапускает трек) и `repeat_launch = true`;
  `launch()` в остальных случаях сбрасывает признак; `Started` при
  `repeat_launch` историю не пишет. Next/Previous/Failed идут по списку
  без перехода по кругу.
- **R6:** `Clear`, `Recover`, `reconcile_sources_in` режимы не меняют.
- `QueueSnapshot` и `PlaybackAssignment` получают `shuffle` и `repeat`.

### HTTP (osu-radio-server)

- `CommandRequest`: `set_shuffle { enabled: bool }`, `set_repeat { mode }`
  (`mode`: `off` | `all` | `one`, snake_case).
- `PlaybackResponse` (ответы команд и строки NDJSON-потока) и `QueueResponse`
  получают `shuffle: bool` и `repeat: "off" | "all" | "one"`. Схемы OpenAPI —
  под фичей `docs`. Маршруты не меняются, поэтому оба варианта роутера остаются
  в синхроне без правок.

### Клиент (osu-radio-client)

- `models.rs`: `RepeatMode`, поля `shuffle`/`repeat` в `PlaybackAssignment`,
  варианты `PlaybackCommand::SetShuffle`/`SetRepeat`.
- `AppCommand::ToggleShuffle` и `AppCommand::CycleRepeat`: `PlayerViewModel`
  вычисляет целевое значение из последнего назначения (Off → All → One → Off)
  и ставит команду в `queue_playback`. Логика цикла остаётся toolkit-free.
- `PlayerPresentation` получает `shuffle`, `repeat` и `can_change_modes`
  (есть соединение и назначение).
- **Previous (R9):** общий метод `PlayerViewModel` для `AppCommand::Previous`
  и системного `Event::Previous`: если текущий запуск звучит в рабочем потоке
  (`system_worker_current`, есть источник) и показанная позиция
  (с учётом скорости, как в `presentation()`) больше 3 с — `playback::Command::Seek`
  в 0 с текущим token, без серверной команды; иначе обычный Previous. Пауза
  и отсутствие `Started` обеспечиваются существующим seek.
- Системные медиаконтролы: `system_snapshot()` передаёт режимы, `system_event()`
  превращает `SetShuffle`/`SetRepeat` в те же команды при готовности.
- Мок (R8): `MockQueue` хранит `shuffle`/`repeat`, новые `Action` переключают их
  офлайн. Мок не перемешивает записи — R8 требует показ и переключение режимов,
  а не повторение серверного алгоритма.

### Qt (osu-radio-qt)

- `PlayerViewModel`: свойства `shuffle`, `repeatMode` (`"off"`/`"all"`/`"one"`),
  `canChangeModes`; invokable `toggleShuffle()`, `cycleRepeat()`.
- `PlayerPanel.qml`: кнопки включены при соединении и `canChangeModes`;
  включённый shuffle и repeat All/One выделяются акцентом (как открытая очередь
  в заголовке); иконка `repeat-2` для Off/All и новая Lucide `repeat-1` для One;
  доступные имена: «Shuffle: off/on», «Repeat: off/all/one».
- Галерея: кнопки режимов, подключённые к мок-действиям через `bridge.rs`.
  Отображение режима (иконка/имя/акцент) вынести в общий QML-компонент,
  используемый панелью и галереей.
- `repeat-1.svg` добавить в ресурсы и манифест native-assets probe.

### Медиаконтролы (osu-radio-media-controls), R10

- Собственный нейтральный тип `Repeat { Off, One, All }`; `Snapshot { shuffle, repeat }`,
  `Capabilities { set_modes }`, `Event::SetShuffle(bool)`, `Event::SetRepeat(Repeat)`.
- MPRIS: `Shuffle` и `LoopStatus` (None/Track/Playlist) из снимка, сеттеры
  отправляют события, изменения публикуются через PropertiesChanged; при
  `!set_modes` сеттеры возвращают ошибку.
- SMTC: `SetShuffleEnabled`/`SetAutoRepeatMode` из снимка; обработчики
  `ShuffleEnabledChangeRequested`/`AutoRepeatModeChangeRequested` → события;
  снятие обработчиков в существующем cleanup.

### Совместимость

Существующие команды и поля не меняются. Клиент читает `shuffle`/`repeat`
с `#[serde(default)]`, как уже сделано для `current_playlist_item_id`: со старым
внешним сервером (`OSU_RADIO_SERVER_URL`) режимы показываются выключенными,
а их команды получат ошибку сервера.

## Крупные этапы

1. **БД:** миграция v16, модель, репозиторий, контракты миграции
   (upgrade с заполненной очередью, reopen, guarded downgrade) на SQLite и PostgreSQL.
2. **Сервис:** команды режимов, shuffle/unshuffle, ключи вставок, запуск плейлиста,
   Repeat All/One, подавление истории, `can_next`. Зависит от 1.
3. **Сервер:** wire-формат команд и ответов, тесты маршрутов. Зависит от 2.
4. **Клиент и мок:** модели, команды, презентация режимов, правило Previous,
   мок галереи. Зависит от 3 (формат).
5. **Qt:** свойства, кнопки, иконка, галерея, обновление probe. Зависит от 4.
6. **Медиаконтролы (отдельная карточка по R10):** API крейта, MPRIS, SMTC,
   связывание в `system_snapshot`/`system_event`. Зависит от 4.
7. **Руководства и сквозная проверка:** `database.md` (v16, ключ порядка,
   `repeat_launch`), `backend.md` (команды и поля), `frontend.md` (режимы, Previous,
   медиаконтролы, галерея), `development.md` (строка проверки); ручная приёмка.

## Проверка результата

| R | Проверка |
| --- | --- |
| R1 | Сервис: `A → [B] → C → D` — shuffle меняет только хвост, current/token/история прежние; выключение восстанавливает порядок, в том числе после `Move`; next/end-вставки при shuffle после выключения сразу после текущей / в конце. Перемешивание проверять по множеству и границам, не по конкретной перестановке. |
| R2 | Клиент: цикл Off → All → One → Off даёт нужные команды. Qt probe: иконка и доступное имя каждого состояния, кнопки включены при соединении. |
| R3 | Сервис: Finished при One перезапускает ту же запись (новый token), `Started` не меняет дату; Next/Previous переходят, режим сохраняется. |
| R4 | Сервис: Finished и Next на последней доступной → первая доступная, порядок не меняется; `can_next` на последней при All. |
| R5 | Сервис: Play из библиотеки / next / end при shuffle; плейлист с явным стартом (старт первый, остальные — все прочие пункты) и без него. |
| R6 | Репозиторий: round-trip и reopen; сервис: `Clear` и `Recover` сохраняют режимы; сервер: поля в ответе и в первой строке потока. |
| R7 | Сервис: Failed при One идёт дальше; при All без доступных записей — Stopped; одна упавшая запись при All не зацикливается. |
| R8 | Мок: переключение режимов офлайн; Qt gallery probe. |
| R9 | Клиент с подменённым движком: позиция 5 с → seek в 0 без HTTP и без `Started`, пауза сохраняется; 2 с → Previous на сервер; первая запись не переходит к последней (сервис). Тот же путь для системного `Event::Previous`. |
| R10 | Тесты адаптеров MPRIS (свойства, сеттеры, PropertiesChanged) и Windows-логики, где она тестируема на Linux; ручная проверка на Linux (`playerctl shuffle`/`loop`) и Windows. |

Проверки по [матрице](../agent/development.md#verification-matrix): repository/service
на SQLite и одноразовой PostgreSQL; `osu-radio-server` с `docs` и без;
`osu-radio-client` с `mock` и без; `osu-radio-media-controls`; Qt offscreen/lint.
Без `--all-features`. Для сборки может понадобиться `LIBCLANG_PATH` (Signalsmith).

Сквозной сценарий (вручную, Linux): включить shuffle при игре, Next по новому
порядку, выключить — порядок восстановлен; Repeat One доигрывает трек и начинает
заново, дата прослушивания не меняется; Repeat All переходит с последней на первую;
Previous на 0:05 и 0:02; перезапуск приложения сохраняет режимы; переключение
из системного виджета. Физический звук, нативные виджеты и Windows — только
вручную; без Windows-машины R10 на Windows остаётся неподтверждённым.

## Готовность к декомпозиции

Существенные решения согласованы. Решения плана, принятые без отдельного вопроса:

- порог Previous сравнивается с показанной позицией (с учётом скорости);
- мок галереи переключает флаги, но не перемешивает список;
- Repeat All оборачивает все пути продвижения вперёд, включая удаление текущей;
- downgrade v16 отказывает при включённых режимах.

Известное ограничение: быстрый двойной клик по кнопке режима до прихода ответа
вычисляет второе значение от прежнего назначения (локальный сервер отвечает
за миллисекунды; не исправляется).

## Карточки реализации

Срез сверки карточек с кодом: `main`, `e0441f4` (2026-10-24); файлы и символы из
плана существуют, противоречий решениям не найдено. T04 повторно сверена после
выполнения T01–T03: зависимости из T02 на месте, в карточку добавлены уточнения
по wire-enum и тестам. Исполнитель перед работой
сверяет актуальные исходники и результаты зависимостей.

| ID | Результат и требования | Зависит от | Карточка | Состояние |
| --- | --- | --- | --- | --- |
| T01 | Миграция v16, модель и репозиторий режимов и ключа порядка. R1, R3, R6. | нет | [T01](shuffle-repeat/T01.md) | выполнено, проверено (SQLite, PostgreSQL) |
| T02 | Команды режимов, shuffle/unshuffle, ключи вставок, запуск плейлиста при shuffle, поля в снимке. R1, R5, R6. | T01 | [T02](shuffle-repeat/T02.md) | выполнено, проверено (SQLite, PostgreSQL) |
| T03 | Repeat All/One, подавление истории при повторе, ошибки, `can_next`. R3, R4, R7, R9 (сервер). | T02 | [T03](shuffle-repeat/T03.md) | выполнено, проверено (SQLite, PostgreSQL) |
| T04 | HTTP: команды `set_shuffle`/`set_repeat`, поля в ответах, NDJSON и OpenAPI. R6. | T02 | [T04](shuffle-repeat/T04.md) | выполнено, проверено (SQLite) |
| T05 | Клиент: модели, `ToggleShuffle`/`CycleRepeat`, презентация, мок. R2, R6, R8. | T04 | [T05](shuffle-repeat/T05.md) | выполнено, проверено (юнит-тесты) |
| T06 | Previous с порогом 3 с для кнопки и системных контролов. R9. | нет (файлы общие с T05) | [T06](shuffle-repeat/T06.md) | выполнено, проверено (юнит-тесты) |
| T07 | Qt: кнопки, иконка `repeat-1`, общий QML-компонент, галерея, пробы. R2, R8. | T05 | [T07](shuffle-repeat/T07.md) | выполнено, проверено (Qt offscreen) |
| T08 | Крейт media-controls: режимы в MPRIS и SMTC. R10. | нет | [T08](shuffle-repeat/T08.md) | выполнено на Linux; SMTC не проверен (до T11) |
| T09 | Подключение режимов клиента к системным контролам. R10. | T05, T08 | [T09](shuffle-repeat/T09.md) | выполнено, проверено (юнит-тесты); нативные виджеты — T11 |
| T10 | Сквозная проверка на настоящем сервере, общий набор проверок и руководства. R1–R10. | T01–T09 | [T10](shuffle-repeat/T10.md) | выполнено, проверено (opt-in пробы на настоящем сервере, scoped-проверки); Previous 3 с — не в пробе |
| T11 | Ручная приёмка Linux/Windows: звук, окно, виджеты. R1–R10. | T10 | [T11](shuffle-repeat/T11.md) | сборка готова, ожидает ручной приёмки пользователем (пункты не проверялись) |

### Покрытие требований

| R | Реализация | Проверка |
| --- | --- | --- |
| R1 | T01, T02 | T02, T10, T11 |
| R2 | T05 (цикл), T07 (иконки и имена) | T05, T07, T11 |
| R3 | T01, T03 | T03, T10, T11 |
| R4 | T03 | T03, T10, T11 |
| R5 | T02 | T02 |
| R6 | T01, T04, T05 | T01, T04, T10, T11 |
| R7 | T03 | T03 |
| R8 | T05, T07 | T05, T07, T11 |
| R9 | T06 (порог), T03 (первая запись) | T03, T06, T10, T11 |
| R10 | T08, T09 | T08, T09, T11 |

### Порядок

Критический путь: T01 → T02 → T04 → T05 → T07 → T10 → T11. Параллельно можно вести:
T03 (после T02, файлы сервиса общие только с T02), T08 (в любой момент), T06 и T09
(после T05: общие `player.rs`/`controller.rs`, выполнять последовательно: T05 → T06
→ T09). T10 стартует после всех остальных.

Для выполнения карточки: «$impl-exec, выполни карточку docs/impl/shuffle-repeat/T01.md;
запиши результаты проверок».
