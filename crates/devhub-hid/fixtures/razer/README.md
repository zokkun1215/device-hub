# Razer 90B feature-report プロトコル — 実機キャプチャ

取得: 2026-11-18 / Windows 11 / DeathAdder V2 X HyperSpeed ドングル (1532:009C) /
`cargo run -p devhub-hid --example razerprobe -- --full`

- 制御チャネル: **if0** (`usage 0001/02`) の 91B feature report (report ID 0x00 + 90B)。
  if1 にも同じ usage のコレクションがあるが feature report を持たない
  (Windows `HidD_SetFeature` が ERROR_INVALID_FUNCTION)。
- レイアウト: `[reportID=00][status][txn=1f][remaining BE16=0][proto=0][size][class][id][args×80][crc][rsvd]`
- CRC = 構造体 bytes[2..=87] (= バッファ 3..=88) の XOR。検証済み (全ファイル)。
- マウス休眠中はドングルが `status=04` (timeout) を返す (`asleep_timeout.resp.bin`)。
  応答データ部は前回のまま残ることがあるので class/id エコーで判定する。

| ファイル | class/id | 意味 | 実機の値 |
|---|---|---|---|
| `get_firmware.req.bin` | 00/81 | ファームウェア取得 (要求) | — |
| `get_firmware.resp.bin` | 00/81 | 〃応答 | **v1.00** (args `01 00`) |
| `get_serial.resp.bin` | 00/82 | シリアル | `632506H22401201` (覚醒直後のみ返すことがあり、状態によってはゼロ埋めになる) |
| `get_dpi_stages.resp.bin` | 04/86 | DPI ステージ | 4 段 (500/600/1600/3200)、現在段 2 (1 ベース) |
| `get_polling.resp.bin` | 00/85 | ポーリング | code `01` = 1000Hz |
| `get_battery.resp.bin` | 07/80 | バッテリー | raw `0x2b` = 43/255 → **17%** |
| `get_idle.resp.bin` | 07/83 | スリープまでの秒 | `01 2c` BE = 300s = 5 分 |
| `get_low_batt.resp.bin` | 07/81 | 低バッテリー閾値 | `0x0d` ≈ 5% (Synapse 書き込み値) |
| `asleep_timeout.resp.bin` | 00/81 | 休眠中の応答 | status `04`・args 全 0 |

充電状態 (07/84) はこのマウスが **status=05 (not supported)** を返す (単三電池のため)。
DPI ステージの応答 `args` = `[varstore=01][active=02][count=04]` + 段ごと 7B
`[番号][X BE16][Y BE16][rsvd×2]` — OpenRazer `razer_chroma_misc_get_dpi_stages` と一致。
