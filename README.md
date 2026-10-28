# Mutabor

Офлайн-конвертер файлов для Windows, macOS и Linux. Выделяешь файл, нажимаешь
**Ctrl+Alt+C** (на Mac **⌘⌥C**), рядом с курсором появляется меню действий для
этого типа файла.

*Offline file converter for Windows, macOS and Linux: select a file, press
Ctrl+Alt+C (⌘⌥C on Mac), pick an action from the menu next to the cursor.
English section below.*

> **Статус: ранняя разработка.** Готовы трей, хоткей и меню. Сами конвертации
> пока не подключены.

## Принципы

- **Всё локально.** Файлы никуда не отправляются. Нет аккаунтов, облака и
  телеметрии.
- **Открытый код.** В репозитории лежит всё, из чего собирается программа.
  Релизы собираются только публично в GitHub Actions — см.
  [Проверка сборки](#проверка-сборки).
- **Исходник не трогается**, если вы сами не выбрали «заменить».

## Что умеет (план v1)

| Тип | Форматы | Действия |
| --- | --- | --- |
| Видео | mp4, mov, avi, mkv, webm, flv, wmv, 3gp | в другой формат, GIF, звук в MP3, сжатие, обрезка, размер, пресеты для Telegram и Discord |
| Аудио | mp3, wav, ogg, flac, m4a, aac, opus, wma | в другой формат, выравнивание громкости |
| Фото | jpg, png, webp, avif, gif, bmp, tiff, ico, heic, svg | в другой формат, поворот, отражение, размер, обрезка, сжатие, удаление GPS и EXIF |
| PDF | pdf | страницы в PNG и JPG, извлечение текста, сжатие |

Документы Word, Excel и PowerPoint не поддерживаются и не планируются.

## Системные требования

Устанавливать ничего не нужно: ffmpeg и остальные утилиты лежат внутри
установщика.

| ОС | Требования |
| --- | --- |
| Windows 10 и 11 | WebView2 — встроен в Windows 11; на Windows 10 установщик поставит его сам, если его нет |
| macOS 11+ | ничего |
| Linux | webkit2gtk — пакет `.deb` подтянет его сам, `.AppImage` содержит всё внутри |

### Первый запуск без подписи

Программа пока не подписана платным сертификатом, поэтому система предупредит:

- **Windows:** «Windows защитила ваш компьютер» → «Подробнее» → «Выполнить в
  любом случае».
- **macOS:** «Не удаётся проверить разработчика» → Системные настройки →
  Конфиденциальность и безопасность → «Всё равно открыть».

## Что программа делает с системой

- Регистрирует глобальный хоткей Ctrl+Alt+C (⌘⌥C).
- Добавляет иконку в трей.
- **Windows:** чтобы узнать выделенные файлы, читает выделение активного окна
  Проводника через стандартный COM-объект `Shell.Application`.
- **macOS:** при первом нажатии хоткея попросит разрешение «управлять Finder» —
  только чтобы узнать выделенные файлы.
- **Сеть:** не используется. (Позже: проверка обновлений на GitHub и докачка
  pandoc — оба отключаемые.)

## Сборка из исходников

Нужны [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 20+ и
[системные зависимости Tauri](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev     # запуск для разработки
npm run tauri build   # установщик для вашей ОС
```

Чтобы работать над меню в обычном браузере, без Rust: `npm run dev` и откройте
`http://localhost:1420/?demo=video` (также `image`, `mixed`, `none`).

## Проверка сборки

Сторонние утилиты не хранятся в репозитории. Их источник, версия и SHA-256
записаны в [`tools/manifest.json`](tools/manifest.json), сборка их скачивает
и сверяет контрольные суммы. Релизы собираются в GitHub Actions с открытым
логом; у каждого файла релиза есть аттестация происхождения и SHA-256 в
описании релиза.

## Лицензия

[GPL-3.0-or-later](LICENSE).

---

## English

Mutabor is an offline file converter. Select files in Explorer or Finder,
press **Ctrl+Alt+C** (**⌘⌥C** on macOS) and pick an action — convert, compress,
rotate, resize — from a popup menu next to the cursor. Nothing leaves your
computer: no accounts, no cloud, no telemetry.

**Status:** early development; tray, hotkey and menu work, conversions are not
wired yet.

Build: `npm install`, then `npm run tauri dev`. Requirements:
[Rust](https://rustup.rs), Node.js 20+, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/).

License: GPL-3.0-or-later.
