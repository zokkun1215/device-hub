# GPUI Device HUB

ブラウザ (www.mouse.fit / hub.xsyd.top) を開きっぱなしにせずにマウス・キーボードを設定する、
Windows 用の個人 GPUI アプリ。デザインは Liquid Glass。

## 構成

```
crates/devhub-core   契約 v2 (model / backend トレイト) + keys + registry + デモバックエンド   ← UI と HID の境界
crates/devhub-hid    実機ドライバ (HID)                                                       ← バックエンド担当
crates/device-hub    GPUI フロントエンド (Liquid Glass)                                         ← フロント担当
vendor/              GPU backdrop を足した gpui-pre / gpui-pre-windows (GPU backdrop パッチ)
docs/NEXT_TASKS.md      次にやること (v4)
docs/LAMZU_MAYA_X_V2_AUDIT2.md  LAMZU MAYA X V2 再監査 (要修正 P1/P2・名前の対応表・作業順)
docs/POLLING_RATE.md    ポーリング実測の検証と改善案 (2700Hz 問題)
docs/OBS_OVERLAY.md     OBS オーバーレイ (デザイン・アプリ側エディタ・設定ファイル連携)
docs/LIVE_LIGHTING.md   ライブライティング (DirectDrive) とキーボード UI 改善 + バックエンド依頼
docs/AUDIO_FRONTEND.md  USB DAC (TP55 / WalkPlay) の EQ 画面と契約の使い方
docs/BACKEND_BRIEF.md   Web HUB 完全移植の機能表 (v3)
docs/ADD_DEVICE.md      新デバイス追加の手順 (別セッション/別エージェント向け)
docs/PROTOCOL_NOTES.md  Web configurator の解析結果 (コマンド表・アドレス・機能一覧)
docs/FRONTEND.md        フロントエンドの構造
```

## 現状 (2026-11-15)

- フロントエンド: 全画面 (ホーム / DPI / ボタン / 性能 / キー割り当て / トリガー / 高度キー / ライティング /
  マクロ / テスト / 一般 / アプリ設定) + 元に戻す・プリセット・コマンドパレット・キャリブレーション・ウィザード +
  サイドバー電池/スリープ表示・磁気ボタン押下深さ・フォント選択 (同梱フォント+永続化)・
  ファームウェア更新通知 (crates/devhub-update)・操作完了トースト。docs/FRONTEND.md。
- NEXT_TASKS v4 の §1–§6 は完了。HTX Ultra 3950 有線 (33e4:5618) も実機検証済み
  (全設定読み・SPDT/ポーリング UI 書き込み・LOD/DPI/ボタン/マクロ — docs/PROTOCOL_NOTES §1.4)。
- 実機検証済 (HTX Ultra 3950 有線、2026-11): SPDT @8 bit0/1 (両接点でクリック判定→遅延低減)、
  LOD 非 index {0.7→3, 1→1, 2→2}、ポーリング 125–8K、競技モード、デバウンス押下/解放/ホイール、
  モーション同期・直線補正・リップル・センサー角度・スリープ・長距離モード。
- 契約: v2 (`Capabilities` + `DeviceState` + 細粒度 `Change` + `HubEvent::Info` で本体側状態変化を通知)。
- devhub-hid: 実装済 — `transport.rs` (feature/output/input report) + `mousefit.rs`
  (feature-report 用に温存、現行機種は使わない) + `mousefit8.rs` (report-8/EEPROM、有線・ドングル共通)
  + `xsyd.rs` (SU66 Max) + `atk.rs` (ATK HUB 系、report-8/EEPROM、CID/MID 機種確定)。
  v1 時代のコードは `legacy/`、解析用に取得した配布原本は Git 管理外（`.private-analysis/`）。
  検証用 example: `enum` (列挙) / `kbdiff` (SU66 差分読み出し) / `lite2e` (ライティング往復) / `tp55e2e` (TP55 EQ 往復)
  / `atkprobe` (ATK 生 EEPROM) / `atkread` (ATK ドライバ経路の読み書き往復)。
- **mouse.fit 経路**: 有線 5219 もドングル 3854 も feature report を持たず
  report-8/EEPROM のみ → 全機種 `mousefit8.rs`。
- 実機検証済 (2026-11-15): HTS Plus Pro 有線+無線 (全設定・DPI・ボタン・マクロ・ポーリング 125–8K・
  磁気ボタン校正・押下深さ・**休眠検知**: コマンドタイムアウト+受信沈黙で asleep、覚醒で自動再読・
  **ShortcutKey は `[count][press][release逆順][crc]` のコンパクト列・RT は en<<7|val RMW、
  トリガーは公式通り 1-20 表示=EEPROM-1** — いずれも実機往復確認 `htse2e`)、
  SU66 Max (キーマップ・ライティング・**キーごと性能・ポーリング・OS/防チャタ/スリープ・
  構成4・高度キー・マクロ・live_travel・キャリブレーション** — いずれも実機で読み書き確認)、
  ATK F1 V3 ULTIMATE+ **有線+ドングル両経路** (CID/MID 検出・FW 1.33・電池・DPI+ステージ色・
  ポーリング 8K・LOD **0.7–1.7mm 0.1刻み実機確認**・モーション同期/直線補正/リップル・
  ホイール/クリックデバウンス・スリープ・移動中LED消灯・bhop・ShortcutKey ボタン Key 読み書き・
  プロファイル4・工場リセット — いずれも往復確認。**ドングル 373b:1278** は同一 16B プロトコル
  中継 + cmd29 ドングル FW + 休眠検知 (asleep/HubEvent・覚醒で CID/MID 再確定)。
  未対応: マクロ @768・dynamicSensitivity・sensorAngle・BT・FW 更新 — §4)。
- **Razer (RAZER SYNAPSE 置換) 追加 (2026-11-18)**: `razer.rs` — 90B feature report
  (transaction 0x1F・CRC bytes[2..87] XOR)、制御は if0 `0001/02` のみ
  (registry に `interface: Some(0)` を追加 — 同 usage が if1 にもあるため)。
  DeathAdder V2 X HyperSpeed (ドングル **1532:009C**) 実機検証済:
  FW/シリアル・DPI 5 段 (100–14000)・ポーリング 125/500/1000・電池 (0-255→%)・
  スリープ 60–900s・低電池閾値 5–25% — 読み書き往復 `razere2e` で確認、
  休眠は status 0x04 で検知 (asleep/HubEvent・覚醒で自動再読)。
  **ボタン割当のデバイス側書き込みは Razer 全機種で未解読** (コミュニティも未 RE) →
  `MouseCaps.buttons` 空で UI 出し分け無し。詳細は PROTOCOL_NOTES §6・fixtures/razer/。
- **常時起動 (アプリ設定→常時起動)**: HKCU `Run` 登録のみ (`autostart.rs`・prefs.autostart・
  起動時 `sync()` で exe 移動後に自己修復)。Synapse プロセス停止ボタン付き。

## ビルド / 実行

```bash
cargo run -p device-hub                  # 実機 (HID が開けなければ自動でデモ)
cargo run --release -p device-hub        # 普段使いは release (起動・描画・メモリが軽い)
DEVHUB_DEMO=1 cargo run -p device-hub    # 常にデモ
DEVHUB_FAST=1 cargo run -p device-hub    # GPU ガラス無効
cargo run -p devhub-hid --example enum   # HID 列挙
cargo run -p devhub-obs --example preview # OBS 無しでオーバーレイ描画を PNG 確認
cargo clippy --workspace
```

## OBS プラグイン (devhub-obs)

`crates/devhub-obs` は cdylib の OBS ソースプラグイン (「Device HUB Live」)。
磁気キー (SU66 `live_axis`) と磁気ボタン (HTS `live_buttons`) の押下深さを
ライブ描画する。obs.dll は実行時に GetProcAddress で解決するので
obs.lib / C++ ツールチェーン無しでビルドできる。

```bash
cargo build --release -p devhub-obs
# インストール (OBS 30.2+ は %APPDATA%\plugins を読まない → ProgramData):
mkdir -p "/c/ProgramData/obs-studio/plugins/devhub_obs/bin/64bit"
cp target/release/devhub_obs.dll "/c/ProgramData/obs-studio/plugins/devhub_obs/bin/64bit/"
# OBS を起動 → ソース追加 → 「Device HUB Live」
# 診断ログ: %TEMP%\devhub_obs.log (create / tex / 初回描画のみ)
# 見た目・表示キーは Device HUB の「OBS オーバーレイ」画面 → %APPDATA%\DeviceHub\obs_overlay.json (docs/OBS_OVERLAY.md)
# デザイン確認: cargo run -p devhub-obs --example gallery
```
```

Rust toolchain は `rust-toolchain.toml` で 1.95.0 固定 (gpui-kit 0.9.1 の要求)。
Linux でも `cargo check` は通る (gpui-pre-wgpu は vendor の backdrop を知らないので、Linux で
ビルドするときだけ `.cargo/config.toml` で wgpu 側に 2 行のパッチが要る — Windows では不要)。

## 守ること

- **UI は caps だけを見て出し分ける**。デバイス名・VID/PID で UI を分岐させない。
  新しいデバイスは registry + ドライバだけで増える。
- スカラーな設定は `SettingSpec`、ボタン 1 つの操作は `ActionSpec` で宣言する (契約を変えずに UI に出る)。
- 契約 (`devhub-core/src/model.rs`) を変えるときは**追加のみ**。demo.rs も更新する。
- `Window::paint_backdrop` は `device-hub/src/glass.rs` 経由でだけ呼ぶ。
- `vendor/` のパッチは上流 (crates.io `gpui-pre`/`gpui-pre-windows` 0.3.11) への最小差分に留める。
  改変箇所には `DEVHUB` のコメントを付ける (`vendor/README.md` 参照)。
- Web 版のタブを開いたまま実機を操作しない (HID を取り合う)。
