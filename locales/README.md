# Bandix translations

The desktop app supports the languages present in the local LuCI Bandix source:
Simplified Chinese, English, Traditional Chinese, Japanese, French, Spanish,
Polish and Russian. English is LuCI's source language; its `po/` directory has
the other seven translation catalogs.

Shared terms are adapted from the Apache-2.0 licensed
[luci-app-bandix translation catalogs](https://github.com/timsaya/luci-app-bandix/tree/main/luci-app-bandix/po).
Desktop-specific messages are included in this directory.

`messages.json` maps stable Simplified Chinese source keys to eight translations.
The array order matches `Language::ALL` in `src/i18n.rs`: `zh-Hans`, `en`,
`zh-Hant`, `ja`, `fr`, `es`, `pl`, `ru`. Numbered placeholders such as `{0}` are
replaced with already-formatted values; translations can reorder them. Never
translate user-provided device names, addresses, domains or protocol values.

`widgets.json` supplies calendar, date-picker, select and input-menu translations
through GPUI Component's rust-i18n backend extension. Both catalogs are embedded
in the binary, so no external translation files are needed at runtime.

Language, speed-unit and refresh-interval preferences are saved together in
`bandix-gui/display.json` under the platform configuration directory on Windows,
Linux and macOS. They are restored at startup. Missing settings preserve the
Simplified Chinese, bytes and one-second defaults; existing configuration files
remain compatible. Changing one setting preserves the others.

Run `python3 scripts/check-translations.py` to check catalog completeness and
placeholder consistency. Run `cargo test` for message formatting and persistence
checks.
