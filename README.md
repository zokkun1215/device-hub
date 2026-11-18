# Device HUB

A native Windows configurator for magnetic-switch keyboards, Hall-effect mice, and
USB DACs — no browser required. Replaces keeping the vendor web configurators
(`mouse.fit`, `hub.xsyd.top`) open.

Built with [GPUI](https://github.com/zed-industries/zed) (Zed's GPU UI framework)
with a Liquid Glass design.

**日本語版は下にあります → [日本語](#日本語)**

![Device HUB](docs/screenshot.png)

## Features

- **Mouse** — DPI stages, polling rate, button remapping (keyboard keys,
  shortcuts, media keys, macros), Hall-effect button calibration, Rapid Trigger
  sensitivity, trigger position, sleep settings, battery display.
- **Magnetic keyboard** — keymap with onboard layers, per-key actuation /
  rapid-trigger / release points, deadzones, lighting, advanced keys
  (DKS/MPT/MT/TGL/SOCD/RS), macros, live travel-depth display, calibration.
- **USB DAC** — parametric EQ editing.
- **Razer (Synapse replacement)** — DPI stages, polling rate, battery, sleep and
  low-battery threshold for Razer wireless mice, speaking the same 90-byte HID
  feature reports Synapse uses — no Razer services, no account, no cloud.
- **Always-on** — optional Windows logon autostart (`HKCU\...\Run` only; no
  services, no background processes beyond the app itself).
- **Process automation** — apply saved profiles when a specific app becomes
  foreground, with automatic restore on leave (multi-device profiles supported).
- **OBS overlay plugin** (`devhub_obs.dll`) — live key/button press-depth source
  that works **fully standalone**: it opens the device itself and every display
  option is configurable from OBS source properties (see `docs/OBS_OVERLAY.md`).
- **Import/export** of settings and profiles, undo, presets, command palette,
  firmware update notifications.

### Verified devices

| Device | Notes |
| --- | --- |
| HTS Plus Pro (wired + 2.4GHz dongle) | All settings incl. Rapid Trigger |
| SU66 Max (Keebforce/Everglide) | Keymap, per-key perf, lighting, macros, live travel |
| TP55 USB DAC | Parametric EQ |
| Razer DeathAdder V2 X HyperSpeed (2.4GHz dongle) | DPI stages, polling rate, battery, sleep, low-battery threshold — Razer Synapse replacement (button remap: not reverse-engineered by the community yet) |

Other models using the same protocols (mouse.fit report-8/EEPROM, xsyd, Razer
90-byte feature reports) are detected automatically by capability, not by
device name.

## Requirements

- Windows 10/11 (x64)
- Rust 1.95.0 (pinned in `rust-toolchain.toml`)

## Build & run

```sh
cargo run --release -p device-hub      # real devices (falls back to demo if none)
DEVHUB_DEMO=1 cargo run -p device-hub  # demo mode, no device needed
cargo clippy --workspace
cargo test --workspace
```

`device-hub.exe` is a single portable binary — fonts and the icon are embedded;
settings live in `%APPDATA%\DeviceHub\`.

### OBS plugin

```sh
cargo build --release -p devhub-obs
```

Copy `target/release/devhub_obs.dll` to
`%ProgramData%\obs-studio\plugins\devhub_obs\bin\64bit\` and the locale files in
`crates/devhub-obs/data/locale/` to `...\devhub_obs\data\locale\`, then restart
OBS and add a "Device HUB Live" source. The plugin does not need the app.

To stage a Windows release with its license files, run these commands in PowerShell:

```powershell
cargo build --release --locked -p device-hub -p devhub-obs
./scripts/package-windows.ps1
```

Archive the contents of `dist/` for the full release or the contents of
`dist/devhub_obs/` for the standalone OBS plugin.

## Repository layout

```
crates/devhub-core    contract: capabilities, device model, backend traits, registry, demo backend
crates/devhub-hid     Windows HID drivers (mouse.fit report-8/EEPROM, xsyd)
crates/device-hub     GPUI frontend (Liquid Glass)
crates/devhub-obs     OBS source plugin + CPU renderer
crates/devhub-update  firmware update notifications
vendor/               gpui-pre / gpui-pre-windows with GPU backdrop patch
docs/                 protocol notes, frontend structure, OBS overlay, add-device guide
```

## Notes

- Do **not** use the vendor web configurator at the same time — they compete for
  the same HID endpoints.
- Rapid Trigger and similar settings write to the device's EEPROM. This is a
  personal project; use at your own risk.
- See `AGENTS.md` / `docs/` for architecture details and how to add a device.

Protocol notes and test captures are kept in [docs/PROTOCOL_NOTES.md](docs/PROTOCOL_NOTES.md) and
[crates/devhub-hid/fixtures](crates/devhub-hid/fixtures). Vendor web bundles are not part of the source distribution.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Bundled fonts (Geist, IBM Plex Sans JP, Zen Maru Gothic) are under the SIL
Open Font License — see `THIRD_PARTY_NOTICES.md`. `vendor/` contains a patched
GPUI (Apache-2.0, © Zed Industries).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion shall be dual-licensed as above, without additional terms.

---

## 日本語

磁気スイッチキーボード・ホール効果マウス・USB DAC を、ブラウザを開かずに設定する
Windows 用ネイティブアプリです (公式 Web 設定ツール `mouse.fit` / `hub.xsyd.top` の代わり)。
UI は GPUI + Liquid Glass デザイン。

### 主な機能

- **マウス** — DPI・ポーリング・ボタン割り当て (キー/ショートカット/メディア/マクロ)・
  磁気ボタン校正・Rapid Trigger・トリガー位置・スリープ・電池表示
- **磁気キーボード** — オンボードレイヤー付きキーマップ・キーごとの AP/RT/リセット・
  デッドゾーン・ライティング・高度キー (DKS/MPT/MT/TGL/SOCD/RS)・マクロ・押下深さライブ表示
- **USB DAC** — パラメトリック EQ
- **Razer (Synapse 置換)** — Razer ワイヤレスマウスの DPI 段・ポーリング・電池・
  スリープ・低バッテリー閾値を、Synapse と同じ 90B HID feature report で直接設定。
  Razer のサービス・アカウント・クラウドは不要
- **常時起動** — Windows ログオン時の自動起動 (HKCU Run 登録のみ。サービスや常駐プロセスは追加しない)
- **プロセス連動** — 指定アプリが前面になったらプロファイルを適用、離れたら復帰 (複数デバイス対応)
- **OBS プラグイン** — 押下深さのライブソース。アプリ無しで単体動作し、設定は OBS プロパティで完結
- 設定・プロファイルのインポート/エクスポート、元に戻す、プリセット、コマンドパレット

### 動作確認デバイス

HTS Plus Pro (有線+ドングル) · SU66 Max · TP55 USB DAC ·
Razer DeathAdder V2 X HyperSpeed (ドングル)
(同プロトコルの他機種も caps ベースで自動検出)

### ビルド

```sh
cargo run --release -p device-hub
```

`device-hub.exe` はフォント・アイコン埋め込みの単体バイナリです。設定は `%APPDATA%\DeviceHub\`。

詳細は `docs/` と `AGENTS.md` を参照してください。
