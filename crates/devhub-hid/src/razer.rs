//! Razer マウス — RAZER SYNAPSE 置換用の 90B feature-report プロトコル。
//!
//! 実装の根拠:
//! - OpenRazer (**GPL-2.0**, `driver/razermouse_driver.c` / `razerchromacommon.c`) の
//!   リバースエンジニアリング成果を Windows HID (feature report) に移植。
//! - DeathAdder V2 X HyperSpeed (ドングル 1532:009C) 実機で読み取り全コマンドを検証
//!   (2026-11-18、`examples/razerprobe`、fixtures/razer/)。
//!
//! # 通信の形
//!
//! - 制御チャネルは **if0 (usage 0001/02) の 91B feature report** (report ID 0x00 + 90B)。
//!   同じ usage のコレクションが if1 にもあるが feature report を持たない
//!   (registry で `interface: Some(0)` に固定)。プロトコル詳細は PROTOCOL_NOTES §6。
//! - 90B レポート: `[status][txn][remaining BE16][proto=0][size][class][id][args×80][crc][rsvd]`
//! - **transaction ID は 0x1F** (DA V2 X HyperSpeed。機種ごとに固定値 — OpenRazer 参照)。
//! - **CRC = bytes[2..=87] の XOR** (status/txn は含まない)。応答でも同じ計算で検証される。
//! - 応答は要求の class/id をエコーし、status 0x02 が成功。0x01=busy、0x03=failure、
//!   **0x04=timeout (マウス休眠中・ドングルのみ応答)**、0x05=not supported。
//! - Linux の OpenRazer と同じく、送信→受信の間に ~31ms の中継待ちを入れる
//!   (`RAZER_NEW_MOUSE_RECEIVER_WAIT_US`)。
//!
//! # コマンド表 (class/id → 機能。すべて実機確認)
//!
//! | class | id  | size | 機能 | 引数 |
//! |---|---|---|---|---|
//! | 0x00 | 0x81 | 2 | ファームウェア取得 | 応答 args[0]=major, [1]=minor |
//! | 0x00 | 0x82 | 0x16 | シリアル取得 | 応答 args = ASCII (NUL 終端) |
//! | 0x00 | 0x85 | 1 | ポーリングレート取得 | 1=1000Hz, 2=500Hz, 8=125Hz |
//! | 0x00 | 0x05 | 1 | ポーリングレート設定 | 同上のコード |
//! | 0x04 | 0x86 | 0x26 | DPI ステージ取得 | 要求 args[0]=1 (VARSTORE) |
//! | 0x04 | 0x06 | 0x26 | DPI ステージ設定 | 下記 |
//! | 0x04 | 0x85 | 7 | 現在ステージ DPI 取得 | 応答 args[1..2]=X BE16, [3..4]=Y BE16 |
//! | 0x07 | 0x80 | 2 | バッテリー取得 | 応答 args[1] = 0..255 |
//! | 0x07 | 0x84 | 2 | 充電状態取得 | **このマウスは 0x05 (非対応) を返す** (単三電池) |
//! | 0x07 | 0x83 | 2 | スリープまでの時間取得 | 応答 args = 秒 BE16 (60..900) |
//! | 0x07 | 0x03 | 2 | スリープまでの時間設定 | 秒 BE16 (60..900 にクランプ) |
//! | 0x07 | 0x81 | 1 | 低バッテリー閾値取得 | 応答 args[0] = 0x0C..0x3F (≈5..25%) |
//! | 0x07 | 0x01 | 1 | 低バッテリー閾値設定 | 同上 |
//!
//! DPI ステージ (get/set 共通): `args = [varstore=1][active(1ベース)][count]`
//! + ステージごと 7B `[番号(1ベース)][X BE16][Y BE16][rsvd][rsvd]` (最大 5 段)。
//! 実機の初期値は 4 段 (500/600/1600/3200)。
//!
//! # 未対応 (わかっている範囲)
//!
//! - **ボタン割り当てのデバイス側書き込みは Razer 全機種で未解読**
//!   (コミュニティの Non-Synapse-Mouse も「Razer's button-mapping command isn't
//!   reverse-engineered yet」と明記)。UI 側のソフトウェアリマップが必要。
//! - 表面キャリブレーション・ドングル FW・BT モード (別 PID)・照明コマンド
//!   (この機種は LED を持たない)。

use devhub_core::backend::*;
use devhub_core::model::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

/// 送信→受信の間隔。OpenRazer の RAZER_NEW_MOUSE_RECEIVER_WAIT_US (31ms) 相当。
const RELAY_DELAY: Duration = Duration::from_millis(35);
/// 応答検証に失敗したときの再試行 (echo 不一致 = 前回応答の残りバッファ等)。
const RETRIES: usize = 5;
const RETRY_GAP: Duration = Duration::from_millis(150);

/// DeathAdder V2 X HyperSpeed の transaction ID (OpenRazer razermouse_driver.c)。
const TXN: u8 = 0x1F;
const ST_BUSY: u8 = 0x01;
const ST_OK: u8 = 0x02;
const ST_TIMEOUT: u8 = 0x04;
const ST_NOT_SUPPORTED: u8 = 0x05;

// ---------------------------------------------------------------- レポート

/// 90B レポート組み立て。`class`/`id`/`size`/`args` から CRC まで埋める。
fn make_report(class: u8, id: u8, size: u8, args: &[u8]) -> [u8; 90] {
    let mut r = [0u8; 90];
    r[1] = TXN;
    r[5] = size;
    r[6] = class;
    r[7] = id;
    let n = args.len().min(80);
    r[8..8 + n].copy_from_slice(&args[..n]);
    r[88] = crc(&r);
    r
}

/// CRC: bytes[2..=87] の XOR (OpenRazer razer_calculate_crc と同じ範囲)。
fn crc(r: &[u8; 90]) -> u8 {
    r[2..88].iter().fold(0u8, |a, b| a ^ b)
}

/// 応答 90B の status (offset 0)。
fn status(r: &[u8; 90]) -> u8 {
    r[0]
}

/// 応答 90B の args スライス (offset 8..88、data_size で切る)。
fn args(r: &[u8; 90]) -> &[u8] {
    let n = (r[5] as usize).min(80);
    &r[8..8 + n]
}

/// バッテリー生値 (0..255) → %。
fn battery_percent(raw: u8) -> u8 {
    ((raw as u32 * 100 + 127) / 255).min(100) as u8
}

/// 低バッテリー閾値 % (5..25) ↔ デバイス値 (0x0C..0x3F ≈ pct×2.55)。
/// OpenRazer の既知 3 点 (5%→12, 15%→38, 25%→63) と Synapse の実測 (5%→13) の
/// どちらでも割り切れるよう、往復は同一式で行う。
fn low_batt_to_byte(pct: i64) -> u8 {
    (pct * 255 / 100).clamp(12, 63) as u8
}

fn low_batt_to_pct(byte: u8) -> i64 {
    (byte as i64 * 100 + 127) / 255
}

// ---------------------------------------------------------------- ドライバ

type Watcher = Arc<dyn Fn(HubEvent) + Send + Sync>;

pub struct RazerDriver {
    dev: Mutex<hidapi::HidDevice>,
    info: Mutex<DeviceInfo>,
    events: Option<Watcher>,
}

/// `info.asleep` を更新し、変わったときだけ `HubEvent::Info` を送る。
fn set_asleep(d: &RazerDriver, asleep: bool) {
    let mut i = d.info.lock().unwrap_or_else(|p| p.into_inner());
    if i.asleep == asleep {
        return;
    }
    i.asleep = asleep;
    let info = i.clone();
    drop(i);
    if let Some(ev) = &d.events {
        ev(HubEvent::Info(info.id.clone(), Box::new(info)));
    }
}

pub fn open(
    dev: hidapi::HidDevice,
    info: DeviceInfo,
    events: Option<Watcher>,
) -> HubResult<RazerDriver> {
    Ok(RazerDriver {
        dev: Mutex::new(dev),
        info: Mutex::new(info),
        events,
    })
}

impl RazerDriver {
    /// feature report 1 往復。応答 90B (report ID 抜き) を返す。
    ///
    /// - 応答は class/id エコー + status 0x02 のものだけを採用する
    ///   (ドングルは前回応答やゼロ埋めを返すことがあるため)。
    /// - status 0x04 はマウス休眠 → `info.asleep` を立てて `Err(Disconnected)`。
    fn xchg(&self, class: u8, id: u8, size: u8, args: &[u8]) -> HubResult<[u8; 90]> {
        let dev = self.dev.lock().unwrap_or_else(|p| p.into_inner());
        let req = make_report(class, id, size, args);
        let mut out = [0u8; 91];
        out[1..].copy_from_slice(&req);
        let mut last = "no response".to_string();
        for attempt in 0..RETRIES {
            if attempt > 0 {
                sleep(RETRY_GAP);
            }
            dev.send_feature_report(&out)
                .map_err(|e| HubError::Io(format!("razer feature write: {e}")))?;
            sleep(RELAY_DELAY);
            let mut resp = [0u8; 91];
            let n = dev
                .get_feature_report(&mut resp)
                .map_err(|e| HubError::Io(format!("razer feature read: {e}")))?;
            if n < 90 {
                last = format!("short response {n}B");
                continue;
            }
            let mut r = [0u8; 90];
            r.copy_from_slice(&resp[1..91]);
            // 前コマンドの残りバッファ (echo 不一致) は再試行。
            if r[6] != class || r[7] != id || r[1] != TXN {
                last = format!(
                    "echo mismatch class={:02x} id={:02x} txn={:02x}",
                    r[6], r[7], r[1]
                );
                continue;
            }
            match status(&r) {
                ST_OK => return Ok(r),
                ST_BUSY => {
                    last = "busy".into();
                    continue;
                }
                // マウス休眠中: ドングル自身が timeout 応答を返す。
                ST_TIMEOUT => {
                    set_asleep(self, true);
                    return Err(HubError::Disconnected);
                }
                ST_NOT_SUPPORTED => {
                    return Err(HubError::Unsupported("このマウスの機能 (非対応コマンド)"));
                }
                s => {
                    last = format!("status {s:02x}");
                    continue;
                }
            }
        }
        Err(HubError::Protocol(format!("razer 0x{class:02x}/0x{id:02x}: {last}")))
    }

    /// DPI ステージ全段 + 現在段 (0 ベース) を読む。
    fn read_stages(&self) -> HubResult<(Vec<DpiStage>, usize)> {
        let r = self.xchg(0x04, 0x86, 0x26, &[0x01])?;
        let a = args(&r);
        if a.len() < 3 {
            return Err(HubError::Protocol("razer: dpi stages が短い".into()));
        }
        let active_1based = a[1] as usize;
        let count = a[2] as usize;
        if !(1..=5).contains(&count) || a.len() < 3 + count * 7 {
            return Err(HubError::Protocol(format!(
                "razer: dpi stages count={count}"
            )));
        }
        let mut stages = Vec::with_capacity(count);
        for i in 0..count {
            let b = &a[3 + i * 7..3 + i * 7 + 7];
            let x = u16::from_be_bytes([b[1], b[2]]) as u32;
            let y = u16::from_be_bytes([b[3], b[4]]) as u32;
            stages.push(DpiStage {
                x,
                y: if y == x { None } else { Some(y) },
                color: Rgb::new(255, 255, 255),
                enabled: true,
            });
        }
        let active = active_1based.saturating_sub(1).min(count - 1);
        Ok((stages, active))
    }

    /// DPI ステージ全段を書く (現在段も一緒に)。値は先に検証する。
    fn write_stages(&self, stages: &[DpiStage], active: usize) -> HubResult<()> {
        if stages.is_empty() || stages.len() > 5 {
            return Err(HubError::Invalid("DPI 段の数 (1〜5)".into()));
        }
        if active >= stages.len() {
            return Err(HubError::Invalid("現在段の番号".into()));
        }
        let mut a = vec![0x01, (active + 1) as u8, stages.len() as u8];
        for (i, s) in stages.iter().enumerate() {
            // UI は 100 刻みで送ってくるが、念のため刻みに丸めて範囲検証する。
            let x = (s.x / 100 * 100).clamp(100, 14000) as u16;
            let y = (s.y.unwrap_or(s.x) / 100 * 100).clamp(100, 14000) as u16;
            a.push((i + 1) as u8);
            a.extend_from_slice(&x.to_be_bytes());
            a.extend_from_slice(&y.to_be_bytes());
            a.extend_from_slice(&[0, 0]);
        }
        a.resize(38, 0); // OpenRazer と同じく 5 段分の宣言サイズ (0x26)
        self.xchg(0x04, 0x06, 0x26, &a)?;
        // フラッシュコミット中に次コマンドを取りこぼすことがあるので少し待つ。
        sleep(Duration::from_millis(120));
        Ok(())
    }

    /// バッテリー生値。休眠中は Err。
    fn battery_raw(&self) -> HubResult<u8> {
        let r = self.xchg(0x07, 0x80, 0x02, &[])?;
        let a = args(&r);
        a.get(1)
            .copied()
            .ok_or_else(|| HubError::Protocol("razer: battery が短い".into()))
    }

    fn caps_static() -> Capabilities {
        Capabilities {
            settings: vec![
                SettingSpec::choice(
                    "polling_wireless",
                    section::PERFORMANCE,
                    "ポーリングレート",
                    vec![
                        ChoiceOpt::new(125, "125 Hz"),
                        ChoiceOpt::new(500, "500 Hz"),
                        ChoiceOpt::new(1000, "1000 Hz"),
                    ],
                )
                .help("無線の報告レート。高いほど追従が速く、電池を多く使います"),
                SettingSpec::slider(
                    "sleep_min",
                    section::POWER,
                    "スリープまでの時間",
                    1.,
                    15.,
                    1.,
                    "分",
                )
                .help("無操作でこの時間が経つとマウスが休眠します (60〜900 秒)"),
                SettingSpec::choice(
                    "low_battery_threshold",
                    section::POWER,
                    "バッテリー残量の警告",
                    vec![
                        ChoiceOpt::new(5, "5%"),
                        ChoiceOpt::new(10, "10%"),
                        ChoiceOpt::new(15, "15%"),
                        ChoiceOpt::new(20, "20%"),
                        ChoiceOpt::new(25, "25%"),
                    ],
                )
                .help("この残量を下回ると本体 LED が赤く点滅します"),
            ],
            actions: vec![],
            profiles: None,
            mouse: Some(MouseCaps {
                dpi_min: 100,
                dpi_max: 14000,
                dpi_step: 100,
                min_stages: 1,
                max_stages: 5,
                // ステージは X/Y を別々に持てるが、この機種の公式 UI は対称のみ。
                xy_split: false,
                // この機種は RGB LED を持たない。
                stage_colors: false,
                stage_toggle: false,
                // ボタン割り当てのデバイス側書き込みは Razer プロトコルで未解読。
                buttons: vec![],
                button_kinds: vec![],
                require_left_click: false,
                macros: None,
                live_buttons: vec![],
                adaptive_dpi: None,
            }),
            keyboard: None,
            lighting: vec![],
            battery: true,
            peq: None,
            firmware_feed: None,
        }
    }
}

impl DeviceDriver for RazerDriver {
    fn info(&self) -> DeviceInfo {
        let mut i = self.info.lock().unwrap_or_else(|p| p.into_inner()).clone();
        // ファームウェア (取得できれば休眠解除とみなす)。
        match self.xchg(0x00, 0x81, 0x02, &[]) {
            Ok(r) => {
                i.asleep = false;
                let a = args(&r);
                i.firmware = Some(format!("v{}.{}", a.first().copied().unwrap_or(0), a.get(1).copied().unwrap_or(0)));
            }
            Err(_) => i.asleep = true,
        }
        // シリアル (取得できたときだけ上書き)。
        if let Ok(r) = self.xchg(0x00, 0x82, 0x16, &[]) {
            let a = args(&r);
            let end = a
                .iter()
                .position(|&b| !(b.is_ascii_graphic()))
                .unwrap_or(a.len());
            if end > 0 {
                i.serial = Some(String::from_utf8_lossy(&a[..end]).into_owned());
            }
        }
        if let Ok(raw) = self.battery_raw() {
            i.battery = Some(Battery {
                percent: Some(battery_percent(raw)),
                charging: false, // 単三電池 — 充電状態コマンドは非対応 (0x05) を返す
            });
        }
        self.info
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .asleep = i.asleep;
        i
    }

    fn caps(&self) -> Capabilities {
        Self::caps_static()
    }

    fn read(&self) -> HubResult<DeviceState> {
        let mut settings = BTreeMap::new();
        // ポーリングレート (未知コードは入れない)。
        if let Ok(r) = self.xchg(0x00, 0x85, 0x01, &[]) {
            if let Some(&code) = args(&r).first() {
                let hz = match code {
                    0x01 => 1000,
                    0x02 => 500,
                    0x08 => 125,
                    _ => 0,
                };
                if hz > 0 {
                    settings.insert("polling_wireless".into(), SettingValue::Choice(hz as i64));
                }
            }
        }
        // スリープまでの時間 (分)。
        if let Ok(r) = self.xchg(0x07, 0x83, 0x02, &[]) {
            let a = args(&r);
            if a.len() >= 2 {
                let secs = u16::from_be_bytes([a[0], a[1]]);
                let mins = (secs / 60) as i64;
                if (1..=15).contains(&mins) {
                    settings.insert("sleep_min".into(), SettingValue::Choice(mins));
                }
            }
        }
        // 低バッテリー閾値 (%。選択肢にスナップ)。
        if let Ok(r) = self.xchg(0x07, 0x81, 0x01, &[]) {
            if let Some(&b) = args(&r).first() {
                let pct = low_batt_to_pct(b);
                let snapped = [5, 10, 15, 20, 25]
                    .into_iter()
                    .min_by_key(|c| (pct - c).abs())
                    .unwrap_or(5);
                settings.insert(
                    "low_battery_threshold".into(),
                    SettingValue::Choice(snapped),
                );
            }
        }
        // DPI ステージ。休眠中はここで失敗する (asleep 表示は Info イベント経由)。
        let (stages, active) = self.read_stages()?;
        set_asleep(self, false);
        Ok(DeviceState {
            profile: ProfileState {
                active: 0,
                names: vec![],
            },
            settings,
            mouse: Some(MouseState {
                stages,
                active_stage: active,
                buttons: vec![],
                macros: vec![],
                adaptive: None,
            }),
            keyboard: None,
            lighting: BTreeMap::new(),
            peq: None,
        })
    }

    fn apply(&self, change: &Change) -> HubResult<()> {
        match change {
            Change::Setting { id, value } => match id.as_str() {
                "polling_wireless" => {
                    let code = match value.as_choice() {
                        125 => 0x08,
                        500 => 0x02,
                        1000 => 0x01,
                        _ => return Err(HubError::Invalid("ポーリングレート".into())),
                    };
                    self.xchg(0x00, 0x05, 0x01, &[code]).map(|_| ())
                }
                "sleep_min" => {
                    let min = value.as_choice().clamp(1, 15);
                    let secs = (min * 60) as u16;
                    self.xchg(0x07, 0x03, 0x02, &secs.to_be_bytes())
                        .map(|_| ())
                }
                "low_battery_threshold" => {
                    let pct = value.as_choice().clamp(5, 25);
                    self.xchg(0x07, 0x01, 0x01, &[low_batt_to_byte(pct)])
                        .map(|_| ())
                }
                _ => Err(HubError::Unsupported("setting")),
            },
            Change::DpiStages { stages, active } => {
                set_asleep(self, false);
                self.write_stages(stages, *active)
            }
            Change::ActiveStage(a) => {
                // 「段だけ切り替える」コマンドは無いので、読み出した全段を
                // 現在段だけ変えて書き戻す (公式も同様に全段を送る)。
                let (stages, _) = self.read_stages()?;
                self.write_stages(&stages, *a)
            }
            _ => Err(HubError::Unsupported("change")),
        }
    }

    fn battery(&self) -> HubResult<Battery> {
        let raw = self.battery_raw()?;
        Ok(Battery {
            percent: Some(battery_percent(raw)),
            // このマウスは充電できない (単三電池)。
            charging: false,
        })
    }
}

// ---------------------------------------------------------------- テスト

#[cfg(test)]
mod tests {
    use super::*;

    /// fixtures/razer/ の実機キャプチャ (91B・report ID 込み) を 90B 構造体へ。
    fn fixture(name: &str) -> [u8; 90] {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("razer")
            .join(name);
        let v = std::fs::read(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(v.len(), 91, "{name} は report ID 込み 91B のはず");
        let mut r = [0u8; 90];
        r.copy_from_slice(&v[1..91]);
        r
    }

    #[test]
    fn crc_matches_device() {
        // 実機応答 (2026-11-18 キャプチャ): CRC バイト [88] は bytes[2..87] の XOR。
        for name in [
            "get_firmware.resp.bin",
            "get_serial.resp.bin",
            "get_dpi_stages.resp.bin",
            "get_polling.resp.bin",
            "get_battery.resp.bin",
            "get_idle.resp.bin",
            "get_low_batt.resp.bin",
            "asleep_timeout.resp.bin",
        ] {
            let r = fixture(name);
            assert_eq!(r[88], crc(&r), "{name} の CRC が実機値と一致しません");
        }
    }

    #[test]
    fn request_builder_matches_capture() {
        // プローブで実機に通った要求と同一のビット列になること。
        let r = make_report(0x00, 0x81, 0x02, &[]);
        assert_eq!(r, fixture("get_firmware.req.bin"));
    }

    #[test]
    fn timeout_response_is_the_asleep_shape() {
        // 休眠中の応答: エコーは正しいが status 0x04 で args が空。
        let r = fixture("asleep_timeout.resp.bin");
        assert_eq!(status(&r), ST_TIMEOUT);
        assert_eq!(r[6], 0x00);
        assert_eq!(r[7], 0x81);
        assert!(args(&r).iter().all(|&b| b == 0));
    }

    #[test]
    fn battery_scale() {
        // 実機: args=00 2b → 0x2b=43 → round(43*100/255)=17%。
        assert_eq!(battery_percent(0x2b), 17);
        assert_eq!(battery_percent(0xff), 100);
        assert_eq!(battery_percent(0), 0);
        let r = fixture("get_battery.resp.bin");
        assert_eq!(battery_percent(args(&r)[1]), 17);
    }

    #[test]
    fn low_battery_roundtrip() {
        // OpenRazer の既知点: 5%→0x0C, 15%→0x26, 25%→0x3F。
        assert_eq!(low_batt_to_byte(5), 0x0c);
        assert_eq!(low_batt_to_byte(15), 0x26);
        assert_eq!(low_batt_to_byte(25), 0x3f);
        // 実機に残っていた Synapse の値 0x0D も 5% に読める。
        assert_eq!(low_batt_to_pct(0x0d), 5);
        assert_eq!(low_batt_to_pct(0x26), 15);
        assert_eq!(low_batt_to_pct(0x3f), 25);
        let r = fixture("get_low_batt.resp.bin");
        assert_eq!(low_batt_to_pct(args(&r)[0]), 5);
    }

    #[test]
    fn parse_dpi_stages() {
        // 実機応答: 4 段 500/600/1600/3200、現在段 = 2 (1 ベース)。
        let r = fixture("get_dpi_stages.resp.bin");
        // 応答内部の整合: class/id エコー。
        assert_eq!(r[6], 0x04);
        assert_eq!(r[7], 0x86);
        assert_eq!(r[88], crc(&r));
        let a = args(&r);
        assert_eq!(a[0], 0x01); // varstore エコー
        assert_eq!(a[1], 0x02); // active (1 ベース)
        assert_eq!(a[2], 0x04); // count
        let vals: Vec<(u16, u16)> = (0..4)
            .map(|i| {
                let b = &a[3 + i * 7..3 + i * 7 + 7];
                (
                    u16::from_be_bytes([b[1], b[2]]),
                    u16::from_be_bytes([b[3], b[4]]),
                )
            })
            .collect();
        assert_eq!(
            vals,
            vec![(500, 500), (600, 600), (1600, 1600), (3200, 3200)]
        );
    }

    #[test]
    fn build_report_matches_probe() {
        // プローブで実機に通った要求と同一のビット列になること。
        let r = make_report(0x00, 0x81, 0x02, &[]);
        let expected: Vec<u8> = "00 1f 00 00 00 02 00 81"
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect();
        assert_eq!(&r[..8], &expected[..]);
        // CRC = size^class^id (残り 0)。
        assert_eq!(r[88], 0x02 ^ 0x00 ^ 0x81);
    }

    #[test]
    fn dpi_write_encoding() {
        // 3 段 (800/1600/3200)、現在段 2 (0 ベース 1) を書くときの args。
        let stages = vec![
            DpiStage {
                x: 800,
                y: None,
                color: Rgb::new(255, 255, 255),
                enabled: true,
            },
            DpiStage {
                x: 1600,
                y: Some(1600),
                color: Rgb::new(255, 255, 255),
                enabled: true,
            },
            DpiStage {
                x: 3200,
                y: None,
                color: Rgb::new(255, 255, 255),
                enabled: true,
            },
        ];
        let mut a = vec![0x01, 0x02, 0x03];
        for (i, s) in stages.iter().enumerate() {
            let x = (s.x as u16).to_be_bytes();
            let y = (s.y.unwrap_or(s.x) as u16).to_be_bytes();
            a.push((i + 1) as u8);
            a.extend_from_slice(&x);
            a.extend_from_slice(&y);
            a.extend_from_slice(&[0, 0]);
        }
        assert_eq!(
            a,
            vec![
                0x01, 0x02, 0x03, // varstore, active(1ベース), count
                0x01, 0x03, 0x20, 0x03, 0x20, 0x00, 0x00, // stage1 800
                0x02, 0x06, 0x40, 0x06, 0x40, 0x00, 0x00, // stage2 1600
                0x03, 0x0c, 0x80, 0x0c, 0x80, 0x00, 0x00, // stage3 3200
            ]
        );
    }
}
