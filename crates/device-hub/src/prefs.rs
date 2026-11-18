//! アプリの設定 (`%APPDATA%\DeviceHub\prefs.json`)。デバイスの設定はデバイス側にあるので、
//! ここに置くのは見た目と UI の状態だけ。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// "dark" | "light"
    pub theme: String,
    pub accent: u8,
    /// フォント (theme::FONT_OPTIONS の id)。
    pub font: String,
    /// GPU ガラス (false で不透明な面)。
    pub glass: bool,
    /// アニメーション。
    pub motion: bool,
    /// 壁紙画像のパス (None = オーロラ)。
    pub wallpaper: Option<String>,
    /// 最後に開いたデバイス。
    pub last_device: Option<String>,
    /// 背面に回ったら描画を止める (ゲーム中に裏で開いていても負荷ゼロ)。
    pub idle_throttle: bool,
    /// 起動時にファームウェア更新を確認する (24 時間に 1 回まで)。
    pub fw_check: bool,
    /// 更新が見つかったら Windows 通知にも出す (既定 OFF)。
    pub fw_native_notify: bool,
    /// Windows ログオン時にこのアプリを自動起動する (常時起動)。
    /// 実体は HKCU Run キー (autostart.rs)。起動時に実レジストリと同期する。
    #[serde(default)]
    pub autostart: bool,
    /// デバイスの別名 (DeviceId → 表示名)。
    pub nicknames: std::collections::BTreeMap<String, String>,
    /// ユーザーが保存した EQ プリセット (USB DAC 共通)。
    pub eq_presets: Vec<EqPreset>,
    /// ライブライティング (DirectDrive) の設定。
    pub livefx: crate::livefx::FxPrefs,
    /// 保存した設定プロファイル (デバイスごとの Change 列)。
    #[serde(default)]
    pub profiles: Vec<SavedProfile>,
    /// プロセス監視ルール (フォアグラウンド連動でプロファイルを適用)。
    #[serde(default)]
    pub proc_rules: Vec<ProcRule>,
}

/// 設定プロファイル — 録画した `Change` を順に再生して適用する。
/// 対象デバイスは複数選べ、デバイスごとに変更列を持つ。
#[derive(Clone, Debug, Serialize)]
pub struct SavedProfile {
    pub name: String,
    /// 対象デバイスごとの変更列。
    #[serde(default)]
    pub devices: Vec<ProfileTarget>,
}

/// プロファイル内の 1 デバイスぶん。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileTarget {
    /// 対象デバイス (DeviceId.0)。
    pub device: String,
    /// 録画/手動で集めた変更 (適用はこの順)。
    #[serde(default)]
    pub changes: Vec<devhub_core::Change>,
}

// 旧形式 (単一 `device` + `changes`) のファイルも読めるよう手動で復元する。
impl<'de> Deserialize<'de> for SavedProfile {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            name: String,
            #[serde(default)]
            devices: Vec<ProfileTarget>,
            // 旧形式フィールド。
            #[serde(default)]
            device: Option<String>,
            #[serde(default)]
            changes: Vec<devhub_core::Change>,
        }
        let r = Repr::deserialize(de)?;
        let mut devices = r.devices;
        if let Some(d) = r.device {
            if !devices.iter().any(|t| t.device == d) {
                devices.push(ProfileTarget {
                    device: d,
                    changes: r.changes,
                });
            }
        }
        Ok(SavedProfile {
            name: r.name,
            devices,
        })
    }
}

/// プロセス監視ルール: exe 名がフォアグラウンドの間プロファイルを適用する。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcRule {
    /// 監視する exe 名 (小文字・部分一致。例: "valorant" / ".exe" なしでも可)。
    pub process: String,
    /// 対象デバイス (DeviceId.0)。
    pub device: String,
    /// フォアグラウンドになったときに適用するプロファイル名。
    pub on_fg: String,
    /// フォアグラウンドを外れたときに適用するプロファイル名 (空 = 何もしない)。
    #[serde(default)]
    pub on_leave: String,
    /// 無効にすると評価しない (適用中に OFF なら on_leave が走る)。
    #[serde(default = "proc_rule_enabled")]
    pub enabled: bool,
}

fn proc_rule_enabled() -> bool {
    true
}

/// 保存した EQ プリセット。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    /// プリアンプ (EQ ボリューム) dB。None なら触らない。
    #[serde(default)]
    pub preamp: Option<f32>,
    pub bands: Vec<devhub_core::model::PeqBand>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            accent: 0,
            font: "modern".into(),
            glass: true,
            motion: true,
            wallpaper: None,
            last_device: None,
            idle_throttle: true,
            fw_check: true,
            fw_native_notify: false,
            autostart: false,
            nicknames: Default::default(),
            eq_presets: vec![],
            livefx: Default::default(),
            profiles: vec![],
            proc_rules: vec![],
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("DeviceHub").join("prefs.json"))
}

impl Prefs {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn light(&self) -> bool {
        self.theme == "light"
    }
}
