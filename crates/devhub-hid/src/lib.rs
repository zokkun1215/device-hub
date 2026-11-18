//! devhub-hid: 実機との会話層。
//!
//! - `connect()` … hidapi を開いて `RealHub` を返す
//! - `RealHub::scan` … `devhub_core::registry::KNOWN` に一致したインターフェースを
//!   `DeviceInfo` として報告し、ID → HID path を `paths` に記録する
//!   (HID で開けないドライバ種別は `supported: false`)。
//! - `RealHub::driver` … ID から記録済み path を開き、ドライバを作ってキャッシュする。
//!   同じ ID には同じ `Arc` を返す (UI はデバイスごとのワーカー 1 本で呼ぶ)。

use devhub_core::backend::*;
use devhub_core::model::*;
use devhub_core::registry;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Arc, Mutex};

// mousefit 新プロトコル (64B feature report) ドライバ — 観測した全機種 (5219 有線・
// 3854 ドングル) は Windows で feature report を持たないため現状どの機種にも振らない。
// JS バンドル解析に基づく実装として残し、feature report を持つ機種が出たらここを使う。
/// ATK 系ドライバ。診断 example (atkread) からも使うので公開。
#[doc(hidden)]
pub mod atk;
#[doc(hidden)]
pub mod lamzu;
/// Razer 系ドライバ (Synapse 置換)。診断 example (razere2e) からも使うので公開。
#[doc(hidden)]
pub mod razer;
#[allow(dead_code)]
mod mousefit;
mod mousefit8;
/// 生 HID チャネル。診断 example (axraw 等) が直接フレームを投げるために公開。
#[doc(hidden)]
pub mod transport;
mod walkplay;
mod xsyd;

/// 実機バックエンドの入口。HID が開けなければ Err (UI はデモへ落ちる)。
pub fn connect() -> HubResult<Arc<dyn HubBackend>> {
    let api = transport::open()?;
    Ok(Arc::new(RealHub {
        api: Mutex::new(api),
        paths: Mutex::new(HashMap::new()),
        drivers: Mutex::new(HashMap::new()),
        names: Mutex::new(HashMap::new()),
        watcher: Mutex::new(None),
        detected: Arc::new(Mutex::new(HashMap::new())),
    }))
}

/// ドライバ種別ごとの実装が済んでいるか。
fn driver_ready(kind: DriverKind) -> bool {
    match kind {
        DriverKind::Xsyd => true,
        DriverKind::Mousefit => true,
        DriverKind::Walkplay => true,
        DriverKind::Atk => true,
        DriverKind::Lamzu => true,
        DriverKind::Razer => true,
        DriverKind::Unsupported => false,
    }
}

pub struct RealHub {
    api: Mutex<hidapi::HidApi>,
    /// `DeviceId.0` → HID path (最後の scan 時点)。
    paths: Mutex<HashMap<String, CString>>,
    /// `DeviceId.0` → ドライバ (オープン済みは使い回す)。
    drivers: Mutex<HashMap<String, Arc<dyn DeviceDriver>>>,
    /// `DeviceId.0` → HID が報告する製品名 (scan 時点で記録)。
    names: Mutex<HashMap<String, String>>,
    /// `DeviceId.0` → ドングル内側機種の確定値 (model, 表示名)。
    /// ドライバが EID で確定した結果を保持し、ドライバ再生成
    /// (切断/invalidate) でも registry 既定へ戻らないようにする。
    detected: Arc<Mutex<HashMap<String, (String, String)>>>,
    /// `watch` で登録された通知窓口 (ドライバへ渡す)。
    watcher: Mutex<Option<Watcher>>,
}

/// ドライバへ渡すイベント窓口。
type Watcher = Arc<dyn Fn(HubEvent) + Send + Sync>;

impl HubBackend for RealHub {
    fn scan(&self) -> HubResult<Vec<DeviceInfo>> {
        let mut api = self.api.lock().unwrap_or_else(|p| p.into_inner());
        api.refresh_devices()
            .map_err(|e| HubError::Io(e.to_string()))?;
        let mut out: Vec<DeviceInfo> = Vec::new();
        let mut paths = self.paths.lock().unwrap_or_else(|p| p.into_inner());
        let mut names = self.names.lock().unwrap_or_else(|p| p.into_inner());
        let prev_paths = std::mem::take(&mut *paths);
        names.clear();
        for d in api.device_list() {
            let Some(spec) = registry::match_device(
                d.vendor_id(),
                d.product_id(),
                d.usage_page(),
                d.usage(),
                d.interface_number(),
            ) else {
                continue;
            };
            let serial = d
                .serial_number()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let id = DeviceId(format!(
                "hid:{:04x}:{:04x}:{}",
                d.vendor_id(),
                d.product_id(),
                serial
                    .clone()
                    .unwrap_or_else(|| format!("{:08x}", fxhash(d.path().to_bytes())))
            ));
            if out.iter().any(|o| o.id == id) {
                continue;
            }
            paths.insert(id.0.clone(), d.path().to_owned());
            // 表示名は HID の製品文字列を優先 (無ければ registry の名前)。
            // ただしドングルは受信機自身の名前を名乗るので、registry (ペア機種名) を優先する。
            let name = match spec.transport {
                Transport::Wireless24G | Transport::Bluetooth => spec.name.into(),
                _ => d
                    .product_string()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| spec.name.into()),
            };
            names.insert(id.0.clone(), name.clone());
            out.push(DeviceInfo {
                id,
                kind: spec.kind,
                name,
                vendor: spec.vendor.into(),
                model: spec.model.into(),
                vid: spec.vid,
                pid: spec.pid,
                transport: spec.transport,
                firmware: None,
                dongle_firmware: None,
                serial,
                driver: spec.driver,
                battery: None,
                supported: driver_ready(spec.driver),
                asleep: false,
                board_id: None,
            });
        }
        // 抜かれた/差し直されたデバイスのキャッシュは死んだハンドル — パスが
        // 前回と一致するものだけ残し、他は捨てて次回 driver() で開き直す。
        self.drivers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|id, _| paths.get(id).is_some() && prev_paths.get(id) == paths.get(id));
        Ok(out)
    }

    fn watch(&self, cb: Box<dyn Fn(HubEvent) + Send + Sync>) {
        *self.watcher.lock().unwrap_or_else(|p| p.into_inner()) = Some(Arc::from(cb));
    }

    fn driver(&self, id: &DeviceId) -> HubResult<Arc<dyn DeviceDriver>> {
        if let Some(d) = self
            .drivers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id.0)
        {
            return Ok(d.clone());
        }
        let path = self
            .paths
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id.0)
            .cloned()
            .ok_or_else(|| HubError::NotFound(id.clone()))?;
        let (vid, pid) = parse_id(id).ok_or_else(|| HubError::NotFound(id.clone()))?;
        let spec = known_spec(vid, pid).ok_or_else(|| HubError::NotFound(id.clone()))?;
        let api = self.api.lock().unwrap_or_else(|p| p.into_inner());
        let dev = api
            .open_path(&path)
            .map_err(|e| HubError::Io(format!("open {}: {e}", spec.name)))?;
        drop(api);
        let name = self
            .names
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id.0)
            .cloned()
            .unwrap_or_else(|| spec.name.into());
        let mut info = DeviceInfo {
            id: id.clone(),
            kind: spec.kind,
            name,
            vendor: spec.vendor.into(),
            model: spec.model.into(),
            vid: spec.vid,
            pid: spec.pid,
            transport: spec.transport,
            firmware: None,
            dongle_firmware: None,
            serial: None,
            driver: spec.driver,
            battery: None,
            supported: true,
            asleep: false,
            board_id: None,
        };
        // ドングルの内側機種が確定済みなら復元 (再生成でも正しい caps で開く)。
        if let Some((m, n)) = self
            .detected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id.0)
        {
            info.model = m.clone();
            info.name = n.clone();
        }
        let events = self
            .watcher
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let drv: Arc<dyn DeviceDriver> = match spec.driver {
            // mouse.fit 機種はドングル経由だけでなく有線 (5219 実機) でも
            // feature report を持たない → 全機種 report-8/EEPROM 経路。
            // 新プロトコル (64B feature report) のドライバは mousefit.rs に残してある。
            DriverKind::Mousefit => {
                Arc::new(mousefit8::open(dev, info, events, self.detected.clone())?)
            }
            DriverKind::Xsyd => Arc::new(xsyd::open(dev, info)?),
            DriverKind::Walkplay => Arc::new(walkplay::open(dev, info)?),
            DriverKind::Atk => Arc::new(atk::open(dev, info, events)?),
            DriverKind::Lamzu => Arc::new(lamzu::open(dev, info, events)?),
            DriverKind::Razer => Arc::new(razer::open(dev, info, events)?),
            DriverKind::Unsupported => {
                return Err(HubError::NotImplemented("このデバイスのドライバ"))
            }
        };
        self.drivers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.0.clone(), drv.clone());
        Ok(drv)
    }

    fn invalidate(&self, id: &DeviceId) {
        self.drivers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id.0);
    }
}

/// `hid:<vid>:<pid>:<serial>` → (vid, pid)。
fn parse_id(id: &DeviceId) -> Option<(u16, u16)> {
    let mut it = id.0.split(':');
    if it.next()? != "hid" {
        return None;
    }
    let vid = u16::from_str_radix(it.next()?, 16).ok()?;
    let pid = u16::from_str_radix(it.next()?, 16).ok()?;
    Some((vid, pid))
}

/// VID/PID → registry エントリ (usage 条件は scan 時点で満たした前提)。
fn known_spec(vid: u16, pid: u16) -> Option<&'static registry::KnownDevice> {
    registry::KNOWN
        .iter()
        .find(|k| k.vid == vid && k.pid == pid && k.vid != 0xFFFF)
}

/// パス文字列の安定ハッシュ (シリアルが無いデバイスの ID 用)。
fn fxhash(b: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for x in b {
        h ^= *x as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}
