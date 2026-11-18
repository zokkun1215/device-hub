# デバイス追加手順 (新しいマウス / キーボードを対応させる)

この手順は **別のセッション・別のエージェント・別の環境** でもそのまま使えるように、
前提知識なしで読めるように書いてある。

- UI (`crates/device-hub`) は**変更不要**。ドライバが `Capabilities` を返せば画面は自動で出る。
- 作業場所は `crates/devhub-core/src/registry.rs` (1 行) と `crates/devhub-hid/src/<driver>/` (プロトコル)。

## 現在の対応表

| デバイス | VID:PID | usage page | ドライバ | Web 版 | 状態 |
|---|---|---|---|---|---|
| SU66 Max | 1CA6:3009 | FFB0 / 01 | Xsyd (`xsyd.rs`) | hub.xsyd.top/keyboard | **v2 実機検証済** (キーマップ・ライティング・per-key 性能・ポーリング・設定・構成・高度キー・マクロ・live_travel・キャリブレーション) |
| HTS Plus Pro (無線) | ドングル 33E4:3854 | FF02 / 02 | Mousefit → `mousefit8.rs` (report-8/EEPROM) | www.mouse.fit | **v2 実機検証済** (全設定・DPI・ボタン・マクロ・ポーリング 125–8K・磁気ボタン校正/押下深さ・休眠検知/覚醒自動再読) |
| HTS Plus Pro (有線) | 33E4:5219 | FF02 / 02 | Mousefit → `mousefit8.rs` (report-8/EEPROM) | www.mouse.fit | **v2 実機検証済** (有線でも feature report は使えず report-8) |
| HTX Ultra | 33E4:5608, 5617, 5618, 5619 | FF02 / 02 (推定) | Mousefit | www.mouse.fit | 機種表から登録 / 未検証 |
| ATK F1 Ultimate | 未確定 | 未確定 | 未確定 | (mouse.fit の機種表に無い) | §7 を参照 |
| DeathAdder V2 X HyperSpeed | ドングル 1532:009C | 0001 / 02 (**if0 固定**) | Razer (`razer.rs`, 90B feature) | (Synapse 専用機種) | **v2 実機検証済** (DPI 5 段・ポーリング 125–1K・電池・スリープ・低電池閾値・FW/シリアル・休眠検知 — §PROTOCOL_NOTES 6、ボタン割当は Razer 未解読のため非対応) |

**接続方式でプロトコルが変わる点に注意**: mouse.fit の新プロトコル (64B feature report) は
有線機種専用 — 8K ドングルは Windows の feature report を一切持たず、ドングル経由は
16B report-8/EEPROM 経路 (`mousefit8.rs`) でしか設定できない。ドングル経由の内側機種は
report-8 では問い合わせられないので、registry の model で機種差を引く (3854 は
ペア機種を仮定する)。

---

## 0. 全体の流れ

1. デバイスを識別する (VID / PID / usage page)
2. Web configurator を解析する (JS を読む + 通信を記録する)
3. registry に登録する → この時点で UI に「検出済み・未対応」と出る
4. ドライバを書く (caps → read → apply → actions)
5. フレームを fixtures に保存してテストを書く
6. 実機で検証し、docs を更新する

## 1. 識別

```bash
cargo run -p devhub-hid --example enum      # 全 HID インターフェースを列挙
```

記録するもの: **VID, PID, product string, usage_page, usage, interface 番号, 接続方式**
(有線 / 2.4G ドングル / 8K ドングルで PID が変わることが多い — すべて記録)。

設定用のインターフェースは vendor-defined (`usage_page >= 0xFF00`) のことがほとんど。
PowerShell なら `Get-PnpDevice -Class HIDClass | ft FriendlyName, InstanceId` でも PID が分かる。

## 2. Web configurator の解析

### 2.1 JS バンドルを手に入れる

Chrome / Edge で configurator を開き、DevTools の Console で:

```js
// 読み込まれた JS と JSON の一覧
performance.getEntriesByType('resource').map(r => r.name).filter(u => /\.(js|json|wasm)/.test(u))
```

Vite 系 (mouse.fit, hub.xsyd.top) は画面ごとにチャンクが分かれている。本体の JS を `fetch` して
`/[\w\-]+\-[\w]{8}\.js/g` で参照されているチャンク名を集め、全部 `fetch` すると全コードが手に入る。
保存したいときは Sources タブで右クリック → Save as、もしくは:

```js
const t = await fetch(URL).then(r => r.text());
const a = Object.assign(document.createElement('a'), {href: URL.createObjectURL(new Blob([t])), download: 'bundle.js'}); a.click();
```

### 2.2 どこを読むか

| 探す文字列 | 分かること |
|---|---|
| `requestDevice` / `filters` / `vendorId` / `usagePage` | 対応 VID/PID・設定用インターフェース |
| `sendFeatureReport` / `receiveFeatureReport` | feature report 型 (mouse.fit 系) |
| `sendReport` / `inputreport` | output/input report 型 (xsyd 系) |
| `reportId` / `new Uint8Array(65)` / `new Uint8Array(64)` | フレーム長 |
| `Crc` / `checksum` / `^=` | チェックサム・難読化 |
| 日本語/英語の UI 文言 (i18n 辞書) | **機能の全一覧** (caps に何を宣言すべきか) |
| `env-models.json` のような設定 JSON | **機種ごとの値の範囲** (caps の値そのもの) |
| `J(this,"getXxx"` / `async getXxx(` | プロトコルクラスのメソッド = 操作の全一覧 |
| `{ReportRate:0, ...}` のような定数オブジェクト | アドレス表・コマンド番号 |

結果は自分の言葉で整理し、出典と実機での確認範囲を添えて `docs/PROTOCOL_NOTES.md` に追記する。
取得した配布ファイルやコードの抜粋は Git 管理外に置く。通信記録をテストに使う場合は、
必要な HID レポートだけを取り出し、シリアル番号や個人設定が含まれていないか確認する。

### 2.3 通信を記録する (フィールド位置の確定)

configurator の JS より先に、次のスニペットを Console に貼ってから「接続」すると、全送受信がログに出る:

```js
(() => {
  const hex = b => [...new Uint8Array(b.buffer ?? b)].map(x => x.toString(16).padStart(2, '0')).join(' ');
  const P = HIDDevice.prototype;
  for (const m of ['sendReport', 'sendFeatureReport']) {
    const o = P[m];
    P[m] = function (id, data) { console.log(`>> ${m} id=${id}`, hex(data)); return o.call(this, id, data); };
  }
  const r = P.receiveFeatureReport;
  P.receiveFeatureReport = async function (id) { const v = await r.call(this, id); console.log(`<< feature id=${id}`, hex(v)); return v; };
  const a = P.addEventListener;
  P.addEventListener = function (t, f, o) {
    if (t === 'inputreport') { const g = f; f = e => { console.log(`<< input id=${e.reportId}`, hex(e.data)); g(e); }; }
    return a.call(this, t, f, o);
  };
  console.log('HID hook installed');
})();
```

**1 項目だけ変えて送信の差分を見る** のが最速 (例: DPI を 800 → 1600 に変えて、どのバイトが変わるか)。
USBPcap + Wireshark でも取れるが、WebHID のフックの方が速い。

### 2.4 フィクスチャとして保存

```
crates/devhub-hid/fixtures/<model>/
  read_dpi.req.bin   read_dpi.resp.bin   read_dpi.md   ← 何のフレームか・何が読めるか
  set_dpi_1600.req.bin ...
```

実機が無い環境・別のエージェントでもテストを書けるようにする。

## 3. registry に登録

`crates/devhub-core/src/registry.rs` の `KNOWN` に 1 行 (接続方式ごとに 1 行ずつ):

```rust
KnownDevice {
    vid: 0x1234, pid: 0x5678,
    usage_page: Some(0xFF00), usage: Some(0x01),
    kind: DeviceKind::Mouse,
    vendor: "Vendor", model: "model-id", name: "表示名",
    transport: Transport::Wired,
    driver: DriverKind::Mousefit,   // 既存プロトコルで話せるならそれ、無ければ §4.3
    verified: false,
},
```

これだけで `cargo run -p device-hub` のサイドバーに「未対応」として出る (`supported=false`)。

## 4. ドライバ

### 4.1 既存プロトコル系列に機種を足す場合

例: mouse.fit の別の G-Wolves マウス → `src/mousefit/models.rs` の機種表に env-models.json の行を足すだけ。
caps は機種表から自動で組み立てる作りにしておく (BACKEND_BRIEF §4)。

### 4.2 caps の書き方

`crates/devhub-core/src/demo.rs` が完全な見本。

- スカラーな設定 → `SettingSpec::toggle / slider / choice` (section を選ぶ)。id は BACKEND_BRIEF §2 の表に合わせる。
- 操作 → `ActionSpec` (危険なら `.danger("確認文")`、ファイルを選ばせるなら `.file(&["bin"])`)。
- マウス → `MouseCaps` (DPI 範囲・ステージ数・ボタン名・割り当て可能な種類・マクロ)。
- キーボード → `KeyboardCaps` (layout は registry に関数を足す・catalog はデバイスのキーコード空間・PerfCaps・高度キー)。
- 照明 → `LightArea` (エフェクト一覧と色/速度/方向の有無)。
- **デバイスに無い機能は宣言しない。** UI はそれで勝手に隠す。

### 4.3 新しいプロトコル系列

1. `crates/devhub-core/src/model.rs` の `DriverKind` に variant を足す。
2. `crates/devhub-hid/src/<name>/mod.rs` に `DeviceDriver` を実装。
3. `crates/devhub-hid/src/lib.rs` の `driver()` と `driver_ready()` に分岐を足す。

### 4.4 実装のチェックリスト

- [ ] `info()` に firmware / dongle firmware / battery
- [ ] `caps()` が Web 版の機能一覧 (i18n 辞書) と一致している
- [ ] `read()` で全項目。読めない項目は map に入れない
- [ ] `apply()` が全 `Change` variant を処理 (未対応は `Err(Unsupported)` ではなく、caps で宣言しない)
- [ ] 必要なら保存コマンド (電源断で戻らないこと)
- [ ] 抜去で `Err(Disconnected)`、無線スリープで `asleep`
- [ ] fixtures を使った単体テスト

## 5. 検証

1. `DEVHUB_DEMO` なしで起動 → デバイスを開く → 全タブを一通り操作。
2. 各値を Web 版で確認 (ブラウザを開くのは検証のときだけ。開いたままアプリを使わない)。
3. 抜き差し・電源再投入で保持されることを確認。
4. registry の `verified: true`、この文書の対応表、PROTOCOL_NOTES を更新。

## 6. よくある落とし穴

- usage page を照合しないと、同じ VID の別インターフェース (キーボード入力そのもの等) を開いてしまう。
- hidapi の write は先頭 1 バイトが report ID (0 でも必要) → 64B フレームは 65B で送る。
- hidapi の read は **unnumbered report では report ID が付かない** (xsyd は buf[0] がデータ先頭、
  report-8 のような numbered では buf[0] が ID)。`transport::Channel::recv` は生バッファを返すので
  プロトコル側で判定する。
- **WebHID の `sendFeatureReport` が通ることと Windows の `HidD_SetFeature` が通ることは別**。
  ドングルのように feature report を持たない IF では Web 版もその経路は使えない
  (JS は try/catch で握り潰しているのでサイト上は「何も起きない」だけ)。実機で
  `send_feature_report`/`get_feature_report` を試してからトランスポートを決めること。
- 応答のデータ開始位置が 1 バイトずれる (mouse.fit は `r[6]` から)。
- 無線ドングル経由では応答が遅い (commonDelay 20ms 程度を入れる)。マウス休眠中は
  ドングルのみが「データ部全 0」で ACK を返す → `DeviceInfo.asleep`。
- 書き込み後の読み戻しで値が丸められていることがある (DPI 刻み・mm×1000 の量子化)。
- 同じ設定でも **接続経路で格納形式が違う**ことがある (mouse.fit の ReportRate@0 は
  インデックスではなく新プロトコルと同じコード表)。Web 版の encode を最後まで追ってから決める。

## 7. 未確定機種 (ATK F1 Ultimate など)

1. 実機を挿して §1 で VID/PID/usage を取る。
2. VID が `33E4` で mouse.fit の機種表に PID があれば Mousefit。無ければ:
   - ATK / VXE 系の configurator (メーカーのサイトから辿る) を開き、§2 で解析する。
   - mouse.fit の「デバイスに接続できませんか？」(LinkJump) の先も確認する。
3. 既存プロトコルと同じなら registry に足すだけ。違えば §4.3。
