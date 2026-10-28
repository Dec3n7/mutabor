# Mutabor

Офлайн-конвертер файлов для Windows, macOS и Linux. Выделяешь файл, нажимаешь
**Ctrl+Alt+C** (на Mac **⌘⌥C**), рядом с курсором появляется меню действий для
этого типа файла.

*Offline file converter for Windows, macOS and Linux: select a file, press
Ctrl+Alt+C (⌘⌥C on Mac), pick an action from the menu next to the cursor.
English section below.*

> **Статус: ранняя разработка.** Работают видео, аудио, фото, PDF и книги.
> Сборки пока только для Windows; готовых установщиков ещё нет.

## Принципы

- **Всё локально.** Файлы никуда не отправляются. Нет аккаунтов, облака и
  телеметрии.
- **Открытый код.** В репозитории лежит всё, из чего собирается программа.
  Релизы собираются только публично в GitHub Actions — см.
  [Проверка сборки](#проверка-сборки).
- **Исходник не трогается**, если вы сами не выбрали «заменить».

## Что умеет

| Тип | Форматы | Действия | Движок |
| --- | --- | --- | --- |
| Видео | mp4, mov, avi, mkv, webm, flv, wmv, 3gp | в MP4, WebM, MOV, MKV, GIF; звук в MP3; сжатие; обрезка по времени; размер; пресеты для Telegram и Discord (≤10 МБ) | FFmpeg, встроен |
| Аудио | mp3, wav, ogg, flac, m4a, aac, opus, wma | в другой формат, обрезка, выравнивание громкости | FFmpeg, встроен |
| Фото | jpg, png, webp, avif, gif, bmp, tiff, ico, heic | в другой формат, в PDF, поворот, отражение, размер, кадрирование по пропорциям, сжатие, удаление GPS и EXIF | FFmpeg, встроен |
| PDF | pdf | объединение, разделение по страницам, страницы в PNG и JPG, текст, сжатие | MuPDF, докачивается |
| Книги и разметка | md, html, epub, fb2, txt | в Markdown, HTML, EPUB, FB2, TXT | Pandoc, докачивается |

У большинства действий есть параметры (Shift+Enter): качество, разрешение,
отрезок времени, пропорции и т. д. Последний выбор запоминается.

Ещё умеет:

- **Сразу много файлов.** Выделите несколько файлов (Ctrl или Shift) или папку
  целиком — действие выполнится для каждого. Файлы ждут в очереди; сколько
  конвертировать одновременно, задаётся в настройках отдельно для видео, аудио,
  картинок, PDF и книг.
- **Результат в буфер обмена.** Кнопка «Копировать» в уведомлении (или
  включите это по умолчанию) — и готовые файлы вставляются в чат через Ctrl+V.
  «Отменить» удаляет только что созданные результаты.
- **Сжать до N МБ** под лимит мессенджера или почты: 8, 10, 16, 25, 50, 100,
  500 МБ.
- **Буфер обмена как источник.** Если ничего не выделено, Mutabor берёт
  скопированные файлы или картинку (скриншот) из буфера. Картинки
  сохраняются в «Изображения\Mutabor». Пока только Windows.
- **Хоткей на любое действие** — например, Ctrl+Alt+D сразу жмёт видео под
  Discord, без меню.
- **Свои действия:** цепочка шагов («убрать звук → сжать до 8 МБ»), шаг может
  быть своей командой ffmpeg. Экспорт и импорт в JSON, чтобы делиться.
- Видео: убрать звук, изменить скорость, кадр в PNG, GIF из отрезка, рингтон
  для iPhone, голосовое для Telegram.

Документы Word, Excel и PowerPoint не поддерживаются и не планируются. SVG
появится позже.

### Докачиваемые движки

MuPDF и Pandoc не входят в установщик, чтобы он оставался маленьким. Действия,
которым они нужны, в меню серые и показывают размер загрузки. При первом
запуске такого действия Mutabor предложит скачать движок с официального сайта,
сверит SHA-256 со значением из [`tools/manifest.json`](tools/manifest.json) и
только потом выполнит действие. Движки можно удалить в настройках.

| Движок | Версия | Лицензия | Загрузка | Системы |
| --- | --- | --- | --- | --- |
| [MuPDF](https://mupdf.com) | 1.28.5 | AGPL-3.0 | 90 МБ | Windows (macOS и Linux позже) |
| [Pandoc](https://pandoc.org) | 3.13 | GPL-2.0+ | 27–43 МБ | Windows, macOS, Linux |

## Системные требования

Устанавливать ничего не нужно: FFmpeg лежит внутри установщика, MuPDF и
Pandoc программа скачает сама, когда понадобятся.

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
- **Сеть:** только когда вы сами нажали «Скачать» для движка — запрос идёт на
  адрес из `tools/manifest.json` (GitHub для Pandoc, mupdf.com для MuPDF).
  Больше программа в интернет не ходит.
- **Файлы:** настройки — `settings.json` в папке настроек (`%APPDATA%\io.github.dec3n7.mutabor`
  на Windows), движки — в `%LOCALAPPDATA%\io.github.dec3n7.mutabor\engines`.
  Результаты сохраняются рядом с исходником с новым именем.

## Сборка из исходников

Нужны [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 20+ и
[системные зависимости Tauri](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev     # запуск для разработки
npm run tauri build   # установщик для вашей ОС
```

Чтобы работать над меню в обычном браузере, без Rust: `npm run dev` и откройте
`http://localhost:1420/?demo=video` (также `image`, `mixed`, `pdf`, `book`, `none`;
`&options=<действие>` открывает его параметры).

Тесты:

```bash
npm run tools -- --optional   # FFmpeg, MuPDF и Pandoc для тестов
cd src-tauri
cargo test                    # быстрые
cargo test -- --ignored       # прогон всех рецептов на настоящих программах
```

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

**Status:** early development. Video, audio, images, PDF and e-books work;
Windows only for now, no installers yet. FFmpeg is bundled; MuPDF (PDF) and
Pandoc (Markdown, HTML, EPUB, FB2) are downloaded from their official sites the
first time you need them and checked against the SHA-256 in
`tools/manifest.json`. That download is the only time Mutabor goes online.

Build: `npm install`, then `npm run tauri dev`. Requirements:
[Rust](https://rustup.rs), Node.js 20+, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/).

License: GPL-3.0-or-later.
