# Localization

Choose **Settings → Language → English / Русский**. The language is saved in
`cloud-provider-settings.json`. Old settings files default to English. Switching
language does not change device configuration, saves, or the console listener.
Terminal commands, prompts, history, and output keep their original text.

## UI messages

`assets/locales/en.json` and `ru.json` contain stable semantic IDs. English text
is a value, never a key. Changing the wording does not require changing an ID.

```json
{
  "language": "ru",
  "name": "Русский",
  "messages": {
    "settings.title": "Настройки",
    "cable.install-guide": "Купите кабель и разъёмы RJ45…",
    "game.saved-path": "Сохранено в {0}"
  }
}
```

UI code explicitly requests a message and passes its arguments:

```rust
tr("settings.title")
tr_args("game.saved-path", &[path.display().to_string()])
```

Templates use numbered arguments (`{0}`, `{1}`). Translations can reorder or
repeat them but must preserve the same argument set as English. Use `{{` and
`}}` for literal braces. Arguments are inserted unchanged; a player name such
as `Settings` is never interpreted as translatable text. Status enums explicitly
select their message IDs. Notifications retain IDs and arguments until rendering,
so switching languages also updates existing notifications. Simulation errors
are mapped by enum variant rather than by their English `Display` output.
Underlying system diagnostic details remain verbatim.

Missing messages fall back to English; an unknown ID renders as `[id]` to expose
mistakes. Invalid JSON, IDs, or mismatched placeholders are logged and skipped.

## Item translations

Item names and descriptions belong to their item configuration, not to the UI
catalog. Drives use `assets/equipment/drives.json`, server components use
`server_parts.json`, rack equipment uses its existing `*_config.json`, and cable
supplies use `supplies.json`. Each item has an ID and language-tagged fields:

```json
{
  "id": "enterprise_ssd_960gb",
  "name": "960 GB Enterprise SATA SSD",
  "display_name": {
    "en": "960 GB Enterprise SATA SSD",
    "ru": "Серверный SATA SSD 960 ГБ"
  },
  "desc": {
    "en": "Enterprise solid-state drive for fast storage.",
    "ru": "Серверный твердотельный накопитель для быстрого хранения данных."
  }
}
```

The existing plain `name` on drives and components is the internal model name
used by simulation and terminals; `display_name` is its UI presentation. Numerical
specifications and model IDs remain independent of translation. UI presentation
is resolved by item ID using the selected language, falling back to that item's
English fields. User-defined names are preserved. The shop displays and searches
localized item names and descriptions.

To add another language, copy an existing locale file, set `language` and its
native `name`, and translate the message values. Add the same language code to
item fields where needed. The Settings selector discovers catalogs at startup.
Runtime JSON overrides are loaded from the asset directory; restart after editing
files. Bundled catalogs and item metadata supply fallback when files are missing.
Portable packages include both locale and equipment directories.

The simulation only owns language-tagged asset metadata; selecting languages,
loading runtime presentation overrides, formatting templates, and rendering text
belong to the game layer. UI frames use immutable scoped language snapshots, so
separate UI/test threads can select their languages independently.
