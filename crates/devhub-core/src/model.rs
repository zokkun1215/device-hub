//! デバイス設定のデータモデル (契約 v2)。
//!
//! フロントエンド (device-hub) とバックエンド (devhub-hid) の**共通言語**。
//! プロトコル上の表現 (バイト列・EEPROM アドレス・コマンド番号) は一切書かない。
//! 変換は各ドライバ (devhub-hid 側) の仕事。
//!
//! # 3 層の考え方
//!
//! 1. **`Capabilities`** — デバイスが「何を持っているか」の静的な説明。UI の出し分けは
//!    すべてこれで決まる。UI はデバイス名・VID/PID で分岐しない。
//! 2. **`DeviceState`** — 今の設定値のスナップショット (`DeviceDriver::read`)。
//! 3. **`Change`** — UI が送る**細粒度の変更 1 件** (`DeviceDriver::apply`)。
//!    Web 版と同じ即時適用。ドライバは受け取った項目だけを書けばよい
//!    (キーマップ全体を毎回書き直す必要はない)。
//!
//! # 汎用設定 (`SettingSpec`)
//!
//! トグル・スライダー・選択肢で表せる「スカラーな設定」(ポーリングレート、LOD、
//! デバウンス、MotionSync、スリープ時間、USB モード…) は **型を増やさず**
//! `SettingSpec` で宣言する。UI は section ごとにカードへ自動で並べる。
//! 新しいデバイスの独自機能は、ドライバが spec を 1 個足すだけで UI に出る。
//!
//! 構造を持つ設定 (DPI ステージ、ボタン割り当て、マクロ、キーマップ、キーごとの
//! トラベル、高度キー、ライティング) だけが専用の型を持つ。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// ====================================================================== 識別

/// 安定したデバイス識別子。HID なら `hid:<vid>:<pid>:<serial or path hash>` を推奨。
/// 再起動しても同じデバイスに同じ ID が付くこと (UI はこれで表示名などを覚える)。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    Mouse,
    Keyboard,
    /// レシーバー単体など。
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transport {
    /// ケーブル直結。
    Wired,
    /// 2.4GHz ドングル経由。
    Wireless24G,
    Bluetooth,
    Unknown,
}

/// プロトコルドライバの種別 (devhub-hid 内の実装モジュールと 1:1)。
/// 新しいプロトコル系列を足すときはここに 1 行足す。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverKind {
    /// hub.xsyd.top 系 (星闪 / Sparklink WebHID SDK)。
    Xsyd,
    /// www.mouse.fit 系 (G-Wolves 等の EEPROM 型プロトコル)。
    Mousefit,
    /// peq.szwalkplay.com 系 (WalkPlay CB 系 USB DAC。report 0x4B S1 プロトコル)。
    Walkplay,
    /// hub.atk.pro / ATK HUB 系 (ATK F1 等。report-8/EEPROM、CID/MID で機種確定)。
    Atk,
    /// www.lamzu.net / Aurora 系 (64B feature report、if2 FFFF:00)。
    Lamzu,
    /// Razer 系 (Synapse 置換)。90B feature report、transaction ID は機種ごとに固定
    /// (OpenRazer driver/razermouse_driver.c の RE 成果の移植 + 実機検証)。
    Razer,
    /// 未割り当て (検出だけできる)。
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    /// 0..=100。不明なら None。
    pub percent: Option<u8>,
    pub charging: bool,
}

/// 検出されたデバイス 1 台分の名札。
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub kind: DeviceKind,
    /// 表示名 ("HTS Plus Pro" など)。
    pub name: String,
    pub vendor: String,
    /// 機種識別子 (registry の model。UI はアイコン選択程度にしか使わない)。
    pub model: String,
    pub vid: u16,
    pub pid: u16,
    pub transport: Transport,
    /// 本体ファームウェア。
    pub firmware: Option<String>,
    /// レシーバー (ドングル) ファームウェア。
    pub dongle_firmware: Option<String>,
    pub serial: Option<String>,
    pub driver: DriverKind,
    /// 検出時に分かっているバッテリー。最新値は `DeviceDriver::battery`。
    pub battery: Option<Battery>,
    /// プロトコル実装が無い/未完成なら false。UI は「検出済み・未対応」と表示する。
    pub supported: bool,
    /// 無線デバイスがスリープ中などで応答しない (ドングルだけ見えている) とき true。
    /// UI は「マウスを動かして起こしてください」と表示する。
    pub asleep: bool,
    /// ボード識別子 (Sparklink 系はファーム配布 API の問い合わせに使う)。
    /// 取れないドライバでは None。
    pub board_id: Option<u32>,
}

// ====================================================================== 色

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    pub const fn hex(v: u32) -> Self {
        Self::new(
            ((v >> 16) & 255) as u8,
            ((v >> 8) & 255) as u8,
            (v & 255) as u8,
        )
    }
    pub fn to_hex(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }
}

// ====================================================================== 汎用設定

/// 汎用設定の値。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SettingValue {
    Bool(bool),
    /// スライダー/数値。単位は spec 側。
    Num(f32),
    /// 選択肢の `ChoiceOpt::value`。
    Choice(i64),
}

impl SettingValue {
    pub fn as_bool(&self) -> bool {
        match self {
            SettingValue::Bool(b) => *b,
            SettingValue::Num(n) => *n != 0.,
            SettingValue::Choice(c) => *c != 0,
        }
    }
    pub fn as_num(&self) -> f32 {
        match self {
            SettingValue::Bool(b) => *b as i32 as f32,
            SettingValue::Num(n) => *n,
            SettingValue::Choice(c) => *c as f32,
        }
    }
    pub fn as_choice(&self) -> i64 {
        match self {
            SettingValue::Bool(b) => *b as i64,
            SettingValue::Num(n) => n.round() as i64,
            SettingValue::Choice(c) => *c,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceOpt {
    pub value: i64,
    pub label: String,
}

impl ChoiceOpt {
    pub fn new(value: i64, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingKind {
    Toggle,
    /// 連続値。`unit` は表示用 ("ms", "mm", "°", "分" など)。
    Slider {
        min: f32,
        max: f32,
        step: f32,
        unit: String,
    },
    /// 選択肢。数が少なければセグメント、多ければドロップダウンで表示される。
    Choice {
        options: Vec<ChoiceOpt>,
    },
}

/// 設定を並べるカード (UI の section)。未知の文字列でも UI は「その他」に並べるだけで落ちない。
pub mod section {
    /// ポーリングレート・USB モード・高速モード等。
    pub const PERFORMANCE: &str = "performance";
    /// LOD・MotionSync・直線補正・リップル・センサー角度等。
    pub const SENSOR: &str = "sensor";
    /// クリック/ホイールのデバウンス、SPDT、誤操作防止。
    pub const CLICK: &str = "click";
    /// 磁気スイッチボタン (トリガー位置・ラピッドトリガー・振動)。
    pub const MAGNETIC: &str = "magnetic";
    /// スリープ・省電力・バッテリー。
    pub const POWER: &str = "power";
    /// 照明の全体スイッチ等 (エフェクトは LightArea)。
    pub const LIGHTING: &str = "lighting";
    /// キーボード全体: OS 種別・Win キーロック・チャタリング防止等。
    pub const KEYBOARD: &str = "keyboard";
    /// 接続・レシーバー。
    pub const CONNECTION: &str = "connection";
    /// USB DAC 系のオーディオ設定 (EQ・フィルター・バランス等)。
    pub const AUDIO: &str = "audio";
    /// それ以外 (リセット・ファームウェア等)。
    pub const SYSTEM: &str = "system";
}

/// 汎用設定 1 件の宣言。
#[derive(Clone, Debug, PartialEq)]
pub struct SettingSpec {
    /// ドライバ内で一意 ("polling_wired", "lod", "motion_sync" …)。`Change::Setting` で戻ってくる。
    pub id: String,
    /// `section::*` のどれか。
    pub section: String,
    pub label: String,
    /// 1 行の説明。
    pub help: Option<String>,
    pub kind: SettingKind,
    /// 他の設定がこの値のときだけ表示する。
    pub visible_when: Option<(String, SettingValue)>,
    /// 変更前に確認ダイアログを出す文言 (危険な設定)。
    pub confirm: Option<String>,
    /// 他の設定がこの値のときだけ操作できる (それ以外は無効表示)。
    /// `(設定 id, 値, 無効時の説明)`。例: 20K FPS は競技モード ON のときだけ。
    pub enabled_when: Option<(String, SettingValue, String)>,
}

impl SettingSpec {
    pub fn toggle(id: &str, section: &str, label: &str) -> Self {
        Self {
            id: id.into(),
            section: section.into(),
            label: label.into(),
            help: None,
            kind: SettingKind::Toggle,
            visible_when: None,
            confirm: None,
            enabled_when: None,
        }
    }
    pub fn slider(
        id: &str,
        section: &str,
        label: &str,
        min: f32,
        max: f32,
        step: f32,
        unit: &str,
    ) -> Self {
        Self {
            kind: SettingKind::Slider {
                min,
                max,
                step,
                unit: unit.into(),
            },
            ..Self::toggle(id, section, label)
        }
    }
    pub fn choice(id: &str, section: &str, label: &str, options: Vec<ChoiceOpt>) -> Self {
        Self {
            kind: SettingKind::Choice { options },
            ..Self::toggle(id, section, label)
        }
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = Some(h.into());
        self
    }
    pub fn when(mut self, id: &str, v: SettingValue) -> Self {
        self.visible_when = Some((id.into(), v));
        self
    }
    pub fn confirm(mut self, msg: &str) -> Self {
        self.confirm = Some(msg.into());
        self
    }
    /// `id` の設定が `v` のときだけ操作可能 (それ以外は `note` を添えて無効表示)。
    pub fn enabled_when(mut self, id: &str, v: SettingValue, note: &str) -> Self {
        self.enabled_when = Some((id.into(), v, note.into()));
        self
    }
}

/// ボタン 1 個で実行する操作 (リセット、ペアリング、キャリブレーション、ファーム更新…)。
///
/// UI の約束 (id の命名規則):
/// - `calibrate` … キーボードで `PerfCaps::live_travel` があると、UI は確認ダイアログの代わりに
///   **キャリブレーション・ウィザード** を開く (開始で `calibrate` → 全キーの底打ちを `live_travel`
///   で見守る → 完了で `calibrate_finish` があれば実行)。
/// - `*_finish` で終わる id … 一覧には出さない (ウィザードの終了処理用)。
/// - 長い操作は実行中に `HubEvent::Progress` を送ると UI が進捗バーを出す。
#[derive(Clone, Debug, PartialEq)]
pub struct ActionSpec {
    pub id: String,
    pub section: String,
    pub label: String,
    pub help: Option<String>,
    /// Some なら実行前に確認ダイアログ。
    pub confirm: Option<String>,
    /// Some なら UI がファイル選択ダイアログを開き、選んだパスを渡す (拡張子フィルタ)。
    pub file: Option<Vec<String>>,
    /// true なら `file` は保存ダイアログ (設定書き出し用)。false = 開く。
    pub save: bool,
    /// 破壊的操作として赤く表示する。
    pub danger: bool,
}

impl ActionSpec {
    pub fn new(id: &str, section: &str, label: &str) -> Self {
        Self {
            id: id.into(),
            section: section.into(),
            label: label.into(),
            help: None,
            confirm: None,
            file: None,
            save: false,
            danger: false,
        }
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = Some(h.into());
        self
    }
    pub fn danger(mut self, confirm: &str) -> Self {
        self.danger = true;
        self.confirm = Some(confirm.into());
        self
    }
    pub fn confirm(mut self, confirm: &str) -> Self {
        self.confirm = Some(confirm.into());
        self
    }
    pub fn file(mut self, exts: &[&str]) -> Self {
        self.file = Some(exts.iter().map(|s| s.to_string()).collect());
        self
    }
    /// `file` と同じだが「名前を付けて保存」ダイアログを開く (設定書き出し)。
    pub fn save_file(mut self, exts: &[&str]) -> Self {
        self.file = Some(exts.iter().map(|s| s.to_string()).collect());
        self.save = true;
        self
    }
}

/// `run_action` の結果。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionOutcome {
    /// トーストに出す文 (None なら既定の「完了しました」)。
    pub message: Option<String>,
    /// true なら UI は `read()` で全設定を読み直す (リセット後など)。
    pub reload: bool,
}

// ====================================================================== 共通: キー・マクロ

/// 修飾キーのビット (HID の modifier byte と同じ並び)。
pub mod mods {
    pub const LCTRL: u8 = 0x01;
    pub const LSHIFT: u8 = 0x02;
    pub const LALT: u8 = 0x04;
    pub const LGUI: u8 = 0x08;
    pub const RCTRL: u8 = 0x10;
    pub const RSHIFT: u8 = 0x20;
    pub const RALT: u8 = 0x40;
    pub const RGUI: u8 = 0x80;
}

/// キーボードのキー 1 打 (+修飾)。`usage` は HID Keyboard/Keypad page (0x07) の usage。
/// 0 は「修飾のみ」。名前は `crate::keys::hid_name`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyStroke {
    pub mods: u8,
    pub usage: u16,
}

/// マウスの基本動作。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseFn {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    DoubleClick,
}

/// メディア/システム (HID Consumer page 0x0C の代表的なもの)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaFn {
    PlayPause,
    Stop,
    Next,
    Prev,
    VolumeUp,
    VolumeDown,
    Mute,
    Calculator,
    MyComputer,
    Browser,
    Mail,
    BrightnessUp,
    BrightnessDown,
    Refresh,
}

/// マクロ 1 イベントの入力。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroInput {
    /// HID usage (keyboard page)。修飾キーも usage (0xE0..0xE7) で表す。
    Key(u16),
    Mouse(MouseFn),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroEdge {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroEvent {
    pub input: MacroInput,
    pub edge: MacroEdge,
    /// このイベントの**後**の待ち時間 (ms)。
    pub delay_ms: u32,
}

/// マクロ 1 本。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroDef {
    pub name: String,
    pub events: Vec<MacroEvent>,
    /// 実行モード (`MacroCaps::modes` の値)。
    pub mode: u8,
    /// 「指定回数」系モードの回数 (`MacroCaps::repeat_modes` のときだけ使う)。
    pub repeat: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacroCaps {
    /// スロット数。
    pub slots: u8,
    /// 1 本あたりの最大イベント数。
    pub max_events: u16,
    /// 最大遅延 (ms)。
    pub max_delay_ms: u32,
    /// 実行モードの一覧 (値, 表示名)。
    pub modes: Vec<ChoiceOpt>,
    /// 回数指定を持つモードの値 (このモードのときだけ repeat を編集させる)。
    pub repeat_modes: Vec<u8>,
    /// マクロ名を保存できるか。
    pub names: bool,
    /// マウスボタンをマクロに入れられるか。
    pub mouse_events: bool,
}

// ====================================================================== プロファイル

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileCaps {
    /// オンボードプロファイル数 (1 なら UI はプロファイル切替を出さない)。
    pub count: u8,
    /// 名前を付けられるか (デバイス側に保存されるか)。
    pub renamable: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileState {
    pub active: u8,
    /// 長さは `ProfileCaps::count`。名前が無ければ空文字。
    pub names: Vec<String>,
}

// ====================================================================== マウス

/// DPI ステージ 1 個。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DpiStage {
    pub x: u32,
    /// XY 分割時の Y。`None` = X と同じ。
    pub y: Option<u32>,
    /// DPI インジケーター LED の色。
    pub color: Rgb,
    /// ステージを無効化 (スキップ) できるデバイス用。
    pub enabled: bool,
}

/// DPI 関連の機能。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DpiFn {
    /// DPI 順送り (ループ)。
    Cycle,
    Up,
    Down,
    /// 押している間だけ指定 DPI (スナイパー / DPI ロック)。
    Shift(u32),
}

/// マクロをボタンで再生するときの動作 (mouse.fit の 3 種)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroRun {
    /// 指定回数。
    Times(u16),
    /// 押している間繰り返す。
    WhileHeld,
    /// もう一度押すまで繰り返す。
    Toggle,
}

/// マウスボタン 1 個の割り当て。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ButtonAction {
    Disabled,
    Mouse(MouseFn),
    Dpi(DpiFn),
    /// 連射: `button` を `count` 回、`interval_ms` 間隔で。
    Fire {
        button: MouseFn,
        count: u8,
        interval_ms: u16,
    },
    /// キーボードのキー / ショートカット。
    Key(KeyStroke),
    Media(MediaFn),
    Macro {
        slot: u8,
        run: MacroRun,
    },
    /// ポーリングレート順送り。
    PollingCycle,
    /// プロファイル順送り。
    ProfileCycle,
    /// 上下ロール (ホイール方向) の切替。
    ScrollModeToggle,
    /// デバイスに記録されているがアプリが解釈できない割当。
    /// 表示と往復保存はできるが、新規割当の候補には出さない。
    Unsupported {
        /// デバイス側の type コード。
        ty: u8,
        /// 生パラメータバイト列。
        params: Vec<u8>,
    },
}

/// ボタン割り当てで選べる種類 (デバイスごとに出し分ける)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonActionKind {
    Disabled,
    Mouse,
    Dpi,
    Fire,
    Key,
    Media,
    Macro,
    PollingCycle,
    ProfileCycle,
    ScrollModeToggle,
}

impl ButtonAction {
    pub fn kind(&self) -> ButtonActionKind {
        match self {
            ButtonAction::Disabled => ButtonActionKind::Disabled,
            ButtonAction::Mouse(_) => ButtonActionKind::Mouse,
            ButtonAction::Dpi(_) => ButtonActionKind::Dpi,
            ButtonAction::Fire { .. } => ButtonActionKind::Fire,
            ButtonAction::Key(_) => ButtonActionKind::Key,
            ButtonAction::Media(_) => ButtonActionKind::Media,
            ButtonAction::Macro { .. } => ButtonActionKind::Macro,
            ButtonAction::PollingCycle => ButtonActionKind::PollingCycle,
            ButtonAction::ProfileCycle => ButtonActionKind::ProfileCycle,
            ButtonAction::ScrollModeToggle => ButtonActionKind::ScrollModeToggle,
            // 候補に出さない: 表示用に Disabled として扱う。
            ButtonAction::Unsupported { .. } => ButtonActionKind::Disabled,
        }
    }
}

/// 物理ボタン 1 個。
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonSlot {
    /// 表示名 ("左ボタン" / "サイド前" …)。
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MouseCaps {
    pub dpi_min: u32,
    pub dpi_max: u32,
    /// DPI の刻み (10/50/100)。
    pub dpi_step: u32,
    pub min_stages: usize,
    pub max_stages: usize,
    /// X/Y 個別 DPI。
    pub xy_split: bool,
    /// ステージごとの LED 色。
    pub stage_colors: bool,
    /// ステージ個別の有効/無効。
    pub stage_toggle: bool,
    /// 物理ボタン (並び = `MouseState::buttons` の並び)。
    pub buttons: Vec<ButtonSlot>,
    /// 割り当て可能な種類。
    pub button_kinds: Vec<ButtonActionKind>,
    /// 「左クリックが最低 1 個必要」を UI でも検査する。
    pub require_left_click: bool,
    pub macros: Option<MacroCaps>,
    /// 押下深さを返せるボタンの slot 番号 (`buttons` の index)。空なら UI は出さない。
    /// `DeviceDriver::live_buttons` の返り値はこの並び。
    pub live_buttons: Vec<usize>,
    /// 速度連動 DPI (AdaDPI / Adaptive DPI) のカーブ。`MouseState::adaptive` と `Change::Adaptive` が対。
    pub adaptive_dpi: Option<AdaptiveCaps>,
}

/// 押下深さを返せるボタン 1 個分 (磁気スイッチなど)。`DeviceDriver::live_buttons`。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ButtonDepth {
    /// 0.0 (離している) 〜 1.0 (底まで)。
    pub depth: f32,
    /// 作動点 0..1 (分かれば)。UI はこの位置に線を引く。
    pub trigger: Option<f32>,
    /// デバイスが「押下」と判定しているか (分かれば)。
    pub pressed: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseState {
    pub stages: Vec<DpiStage>,
    pub active_stage: usize,
    /// `MouseCaps::buttons` と同じ並び。
    pub buttons: Vec<ButtonAction>,
    /// スロット順。空スロットは None。
    pub macros: Vec<Option<MacroDef>>,
    /// 速度連動 DPI のカーブ (`MouseCaps::adaptive_dpi` があるときだけ Some)。
    #[serde(default)]
    pub adaptive: Option<AdaptiveCurve>,
}

/// 速度連動 DPI カーブの 1 点: マウス速度 → DPI 倍率。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdaptivePoint {
    /// 速度 (デバイス単位。LAMZU は 0–105 の目盛り)。
    pub speed: u16,
    /// DPI 倍率 (1.0 = 等倍)。
    pub ratio: f32,
}

/// 速度連動 DPI の状態。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveCurve {
    pub enabled: bool,
    /// 速度の昇順。
    pub points: Vec<AdaptivePoint>,
}

/// 速度連動 DPI の機能宣言。
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveCaps {
    pub min_points: usize,
    pub max_points: usize,
    pub speed_max: u16,
    pub ratio_max: f32,
    /// 公式プリセット (名前, 点列)。
    pub presets: Vec<(String, Vec<AdaptivePoint>)>,
}

// ====================================================================== キーボード

/// キー 1 個の物理配置。
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutKey {
    /// プロトコル上のキー識別 (xsyd はマトリクス `(row<<4)|col`)。
    /// `KeyboardState` の配列はすべて **layout の並び順** で、この値はドライバの参照用。
    pub id: u16,
    /// 左上座標と大きさ (1u 単位)。
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 既定の刻印。
    pub label: String,
}

/// 割り当て候補 1 件 (キーボード)。
#[derive(Clone, Debug, PartialEq)]
pub struct KeyChoice {
    /// デバイス固有のキーコード (そのまま `KeyAction::Code` に入る)。
    pub code: u32,
    /// 一覧に出す名前。
    pub label: String,
    /// キーキャップに出す短い名前 (無ければ label)。
    pub short: Option<String>,
}

/// 割り当て候補のグループ ("基本" / "マウス" / "メディア" / "ライティング" / "レイヤー" / "システム" / "ゲームパッド")。
#[derive(Clone, Debug, PartialEq)]
pub struct KeyGroup {
    pub name: String,
    pub keys: Vec<KeyChoice>,
}

/// キーに割り当てる動作。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAction {
    /// 無効 (何も出さない)。
    None,
    /// 透過 (下のレイヤーを使う)。
    Transparent,
    /// `KeyboardCaps::catalog` のコード。
    Code(u32),
    /// 修飾 + コード (組み合わせキー)。
    Combo { mods: u8, code: u32 },
    /// マクロスロット。
    Macro(u8),
}

/// トリガー方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMode {
    /// 固定アクチュエーション。
    Normal,
    /// ラピッドトリガー。
    Rapid,
}

/// キー 1 個の磁気スイッチ設定 (長さはすべて mm)。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyPerf {
    pub mode: TriggerMode,
    /// 押下で ON になる深さ。
    pub actuation: f32,
    /// 戻りで OFF になる深さ (None = actuation と同じ)。
    pub release: Option<f32>,
    /// RT 有効時: 最初の ON になる深さ (First touch)。
    pub rt_first: f32,
    /// RT: さらに押し込んで再 ON になる量。
    pub rt_press: f32,
    /// RT: 戻して OFF になる量。
    pub rt_release: f32,
    /// 上端デッドゾーン。
    pub top_deadzone: f32,
    /// 下端デッドゾーン。
    pub bottom_deadzone: f32,
    /// 連続 RT (完全に戻るまで RT を継続)。
    pub continuous: bool,
    /// スイッチ種別 (`PerfCaps::switch_types` の index)。
    pub switch_type: u8,
}

impl Default for KeyPerf {
    fn default() -> Self {
        Self {
            mode: TriggerMode::Normal,
            actuation: 1.2,
            release: None,
            rt_first: 0.3,
            rt_press: 0.15,
            rt_release: 0.15,
            top_deadzone: 0.1,
            bottom_deadzone: 0.1,
            continuous: false,
            switch_type: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PerfCaps {
    /// 全ストローク (mm)。
    pub travel: f32,
    /// 設定できる最小値 (mm)。
    pub min: f32,
    /// 刻み (RT 精度、0.01 / 0.001 mm)。
    pub step: f32,
    pub rapid_trigger: bool,
    /// アクチュエーションと別にリリース点を持てるか。
    pub separate_release: bool,
    /// RT の First touch を持つか。
    pub rt_first: bool,
    pub deadzones: bool,
    pub continuous_rt: bool,
    /// スイッチ種別 (軸) の名前一覧。空なら UI は出さない。
    pub switch_types: Vec<String>,
    /// `switch_types` と同じ並びのベンダー軸 ID (軸名をオンラインで引く鍵)。任意。
    pub switch_ids: Vec<u16>,
    /// キーごとに設定できるか (false なら全キー一括)。
    pub per_key: bool,
    /// 押下深さのリアルタイム表示 (`DeviceDriver::live_travel`) ができるか。
    pub live_travel: bool,
}

/// ストローク計測の生データ 1 キー分 (`DeviceDriver::live_axis`)。
/// ドライバが提供できる項目だけ埋める (未対応フィールドは 0/false のまま)。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisSample {
    /// 押下深さ mm (`live_travel` と同じ値)。
    pub route: f32,
    /// 生 ADC 値 (ホールセンサー等)。未対応なら 0。
    pub adc: u16,
    /// キー状態フラグ (0=離し / 非 0=押し中等、機種依存)。
    pub status: u8,
    /// キャリブレーション状態 (0=未 / 非 0=済 等、機種依存)。
    pub calibrated: u8,
}

/// DKS の 1 フェーズでの動作。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DksPhase {
    #[default]
    Off,
    /// そのフェーズで押して離す。
    Tap,
    /// そのフェーズから次の区切りまで押しっぱなし。
    Hold,
}

/// DKS の 1 行 (割り当てキー 1 個 × 4 フェーズ)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DksRow {
    pub code: u32,
    /// [押し始め, 底打ち, 戻り始め, 完全リリース]
    pub phases: [DksPhase; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SocdMode {
    /// 後入力優先 (上書き)。
    Last,
    /// 先入力優先。
    First,
    /// A 優先。
    PriorityA,
    /// B 優先。
    PriorityB,
    /// 両方キャンセル。
    Neutral,
}

/// 高度キー (Sparklink の DKS/MPT/MT/TGL/END/SOCD/RS)。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Advanced {
    /// Dynamic Keystroke: 最大 4 キー × 4 フェーズ。`bottom` は底打ち判定の深さ (mm)。
    Dks { rows: Vec<DksRow>, bottom: f32 },
    /// 多段トリガー: 3 つの深さで別々のキー (深さは単調増加)。
    Mpt { depths: [f32; 3], codes: [u32; 3] },
    /// タップ/長押し。
    Mt { tap: u32, hold: u32, hold_ms: u16 },
    /// トグル。`delay_ms` は公式 wire の `time` (トグル出力の押下時間)。
    Tgl { code: u32, delay_ms: u16 },
    /// エンドトリガー: 押すと `code`、離すと `code_release` を `delay_ms` だけ出す。
    /// 公式 wire `kcs:[press,release]` に対応 (`point` は wire に存在しない)。
    End {
        code: u32,
        code_release: u32,
        delay_ms: u16,
    },
    /// SOCD: 自分 (A) と `other` (B, layout index) のペア。
    /// `delay_ms` は公式 wire の `delay` (反転判定の遅延)。
    Socd {
        other: usize,
        code_a: u32,
        code_b: u32,
        mode: SocdMode,
        delay_ms: u16,
    },
    /// ラピッドスナップ: 2 キーのうち深く押された方。
    Rs {
        other: usize,
        code_a: u32,
        code_b: u32,
        delay_ms: u16,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdvancedKind {
    Dks,
    Mpt,
    Mt,
    Tgl,
    End,
    Socd,
    Rs,
}

impl Advanced {
    pub fn kind(&self) -> AdvancedKind {
        match self {
            Advanced::Dks { .. } => AdvancedKind::Dks,
            Advanced::Mpt { .. } => AdvancedKind::Mpt,
            Advanced::Mt { .. } => AdvancedKind::Mt,
            Advanced::Tgl { .. } => AdvancedKind::Tgl,
            Advanced::End { .. } => AdvancedKind::End,
            Advanced::Socd { .. } => AdvancedKind::Socd,
            Advanced::Rs { .. } => AdvancedKind::Rs,
        }
    }
}

/// レイアウト index (`KeyboardCaps::layout` の並び) に付いた高度キー。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdvancedKey {
    pub key: usize,
    pub layer: u8,
    pub binding: Advanced,
}

/// 照明エフェクト 1 種の宣言。
#[derive(Clone, Debug, PartialEq)]
pub struct LightEffect {
    pub id: u8,
    pub name: String,
    /// このエフェクトで色を選べるか。
    pub color: bool,
    pub speed: bool,
    pub direction: bool,
    /// キーごとの色 (カスタム) モードか。
    pub per_key: bool,
}

impl LightEffect {
    pub fn new(id: u8, name: &str) -> Self {
        Self {
            id,
            name: name.into(),
            color: false,
            speed: true,
            direction: false,
            per_key: false,
        }
    }
    pub fn color(mut self) -> Self {
        self.color = true;
        self
    }
    pub fn still(mut self) -> Self {
        self.speed = false;
        self
    }
    pub fn dir(mut self) -> Self {
        self.direction = true;
        self
    }
    pub fn per_key(mut self) -> Self {
        self.per_key = true;
        self.speed = false;
        self
    }
}

/// 照明の領域 (本体キー / デコレーション / マウスの LED / レシーバー)。
#[derive(Clone, Debug, PartialEq)]
pub struct LightArea {
    pub id: u8,
    pub name: String,
    pub effects: Vec<LightEffect>,
    pub brightness_max: u8,
    pub speed_max: u8,
    /// 方向の選択肢 (空なら無し)。
    pub directions: Vec<String>,
    /// 全体 ON/OFF を持つか。
    pub switchable: bool,
    /// 上下分割ライトのゾーン名 (例: ["上部ライト", "下部ライト"])。
    /// 空なら単一ゾーン。非空なら `LightingState.zone_on` が各ゾーンの開閉。
    pub zones: Vec<String>,
    /// キーごとのカスタム色を持てるか (キーボードエリアのみ)。
    pub per_key: bool,
    /// DirectDrive (per-LED ライブ書き込み `[5,5]`) に対応するか。
    /// true のとき UI は「実機でプレビュー」トグルを出し、点灯イメージを
    /// そのまま実機 LED に流せる (永続しない・停止でファーム駆動に戻る)。
    pub direct_drive: bool,
    /// ライブ点灯色の読み戻し (`DeviceDriver::live_lighting`) に対応するか。
    /// true のとき UI は表示中に実機の現在色をポーリングしてプレビューに使う。
    /// false でもドライバが `Unsupported` を返すだけで無害だが、caps だけ見る
    /// 原則に揃えるため明示する。
    pub live_readback: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightingState {
    /// 点灯しているか (どれかのゾーンが開いていれば true)。
    pub on: bool,
    pub effect: u8,
    /// 選択中パレットスロットの色 (`palette[palette_sel]`)。
    pub color: Rgb,
    pub brightness: u8,
    pub speed: u8,
    pub direction: u8,
    /// カスタム色オーバーレイのキーごとの色 (レイアウト index → 色)。
    /// ベースエフェクトとは独立 — isCustom セルはどのモードでも上に重なる。
    pub per_key: BTreeMap<usize, Rgb>,
    /// 上下ゾーンごとの開閉 (`LightArea.zones` と同じ並び。空なら `on` だけ見る)。
    #[serde(default)]
    pub zone_on: Vec<bool>,
    /// パレットの全スロット色 (空ならパレット非対応)。
    #[serde(default)]
    pub palette: Vec<Rgb>,
    /// 選択中のパレットスロット index。
    #[serde(default)]
    pub palette_sel: u8,
    /// 彩度 (ColorCorrection, 公式の saturation)。`[5,1,area,2]` で読める
    /// 機種のみ Some — R,G,B 各 0-100。None なら UI は出さない。
    #[serde(default)]
    pub color_correction: Option<Rgb>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyboardCaps {
    pub layout: Vec<LayoutKey>,
    /// レイヤー名 ("メイン", "Fn1" …)。
    pub layers: Vec<String>,
    /// 割り当て候補 (グループ順に UI に並ぶ)。
    pub catalog: Vec<KeyGroup>,
    /// 組み合わせキー (修飾 + キー) を割り当てられるか。
    pub combos: bool,
    pub perf: Option<PerfCaps>,
    /// 使える高度キー。
    pub advanced: Vec<AdvancedKind>,
    pub macros: Option<MacroCaps>,
}

impl KeyboardCaps {
    /// コードの表示名 (catalog から引く)。
    pub fn code_label(&self, code: u32) -> Option<(&str, Option<&str>)> {
        self.catalog
            .iter()
            .flat_map(|g| g.keys.iter())
            .find(|k| k.code == code)
            .map(|k| (k.label.as_str(), k.short.as_deref()))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyboardState {
    /// `[layer][layout index]`。
    pub keymap: Vec<Vec<KeyAction>>,
    /// layout index 順。
    pub perf: Vec<KeyPerf>,
    pub advanced: Vec<AdvancedKey>,
    pub macros: Vec<Option<MacroDef>>,
}

// ====================================================================== オーディオ (USB DAC)

/// EQ バンドのフィルター種別 (UI 上の意味。wire index への変換はドライバ)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PeqFilter {
    LowShelf,
    Peak,
    HighShelf,
    LowPass,
    HighPass,
}

impl PeqFilter {
    pub fn label(self) -> &'static str {
        match self {
            PeqFilter::LowShelf => "LS",
            PeqFilter::Peak => "PK",
            PeqFilter::HighShelf => "HS",
            PeqFilter::LowPass => "LP",
            PeqFilter::HighPass => "HP",
        }
    }
}

/// パラメトリック EQ のバンド 1 個。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeqBand {
    /// 中心周波数 Hz。
    pub freq: u16,
    /// Q 値 (フィルターが Q を使わない種別でも保持する)。
    pub q: f32,
    /// ゲイン dB。
    pub gain: f32,
    /// フィルター種別 (`PeqCaps::filters` に含まれる値)。
    pub filter: PeqFilter,
}

/// パラメトリック EQ の機能宣言。UI は `peq` が Some のとき EQ エディタを出す。
#[derive(Clone, Debug, PartialEq)]
pub struct PeqCaps {
    /// バンド数。
    pub bands: u8,
    /// 周波数の範囲 (Hz)。
    pub freq_min: u16,
    pub freq_max: u16,
    /// Q の範囲。
    pub q_min: f32,
    pub q_max: f32,
    /// ゲインの範囲 (dB)。
    pub gain_min: f32,
    pub gain_max: f32,
    /// 使えるフィルター種別。
    pub filters: Vec<PeqFilter>,
}

// ====================================================================== 全体

/// デバイスが持つ機能の全体像。
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    pub settings: Vec<SettingSpec>,
    pub actions: Vec<ActionSpec>,
    pub profiles: Option<ProfileCaps>,
    pub mouse: Option<MouseCaps>,
    pub keyboard: Option<KeyboardCaps>,
    /// 照明の領域 (マウス・キーボード共通)。
    pub lighting: Vec<LightArea>,
    /// バッテリーを持つ (サイドバーに残量を出すかの判定に使う)。
    pub battery: bool,
    /// パラメトリック EQ (USB DAC 等)。`DeviceState::peq` と `Change::Peq*` が対になる。
    pub peq: Option<PeqCaps>,
    /// 公式ファームウェアの配布元 (更新通知用)。無い機種は None。
    pub firmware_feed: Option<FirmwareFeed>,
}

/// 公式ファームウェアの配布元 (更新の「通知」だけ。書き込みは行わない)。
#[derive(Clone, Debug, PartialEq)]
pub enum FirmwareFeed {
    /// mouse.fit: `Config/<ui_folder>/FWLINK.txt` に zip の URL 1 行。
    MouseFit { ui_folder: String },
    /// Sparklink: `api.sparklinkplayjoy.com/api/v1/getKeyboardFirmware`。
    /// board_id はデバイスの DEVICE_INFO 応答から取る (`DeviceInfo::board_id`)。
    Sparklink {
        vid: u16,
        pid: u16,
        board_id: Option<u32>,
    },
}

impl Capabilities {
    pub fn empty() -> Self {
        Self {
            settings: vec![],
            actions: vec![],
            profiles: None,
            mouse: None,
            keyboard: None,
            lighting: vec![],
            battery: false,
            peq: None,
            firmware_feed: None,
        }
    }
    pub fn setting(&self, id: &str) -> Option<&SettingSpec> {
        self.settings.iter().find(|s| s.id == id)
    }
}

/// 全設定のスナップショット。serde でそのままファイルへ書き出せる (UI の「書き出し/読み込み」)。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceState {
    pub profile: ProfileState,
    /// `SettingSpec::id` → 値。読めなかった設定は入れない (UI はその行を無効表示)。
    pub settings: BTreeMap<String, SettingValue>,
    pub mouse: Option<MouseState>,
    pub keyboard: Option<KeyboardState>,
    /// `LightArea::id` → 状態。
    pub lighting: BTreeMap<u8, LightingState>,
    /// `PeqCaps::bands` と同じ並びのバンド。`Capabilities::peq` があるときだけ Some。
    #[serde(default)]
    pub peq: Option<Vec<PeqBand>>,
}

/// UI → ドライバの細粒度な変更。ドライバは**これ 1 件分だけ**を書けばよい。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Change {
    /// 汎用設定。
    Setting {
        id: String,
        value: SettingValue,
    },
    /// オンボードプロファイル切替。成功後 UI は `read()` で読み直す。
    Profile(u8),
    ProfileName {
        index: u8,
        name: String,
    },

    // ---- マウス
    /// DPI ステージ全体 (数・値・色) と現在ステージ。
    DpiStages {
        stages: Vec<DpiStage>,
        active: usize,
    },
    /// 現在ステージだけ切替。
    ActiveStage(usize),
    Button {
        slot: usize,
        action: ButtonAction,
    },

    // ---- 共通
    /// マクロ 1 本 (None で削除)。マウスなら `MouseState::macros`、キーボードなら `KeyboardState::macros`。
    Macro {
        slot: usize,
        def: Option<MacroDef>,
    },
    Lighting {
        area: u8,
        state: LightingState,
    },

    // ---- キーボード
    KeyMap {
        layer: u8,
        key: usize,
        action: KeyAction,
    },
    /// そのレイヤーを既定のキーマップに戻す。
    KeyMapReset {
        layer: u8,
    },
    /// 複数キーに同じトラベル設定。
    KeyPerf {
        keys: Vec<usize>,
        perf: KeyPerf,
    },
    /// 高度キーの設定/解除 (binding None で解除)。
    Advanced {
        key: usize,
        layer: u8,
        binding: Option<Advanced>,
    },

    // ---- オーディオ (USB DAC)
    /// EQ のバンド 1 個。
    Peq {
        band: usize,
        value: PeqBand,
    },
    /// EQ 全バンドを一括 (プリセット適用・インポート/undo 用)。
    PeqAll {
        bands: Vec<PeqBand>,
    },

    // ---- マウス (速度連動 DPI)
    /// 速度連動 DPI の有効/無効とカーブ。
    Adaptive(AdaptiveCurve),
}

impl Change {
    /// 同じ対象への変更か (UI のキューで古い方を捨てる = ドラッグ中の連続送信を間引く)。
    pub fn same_target(&self, other: &Change) -> bool {
        use Change::*;
        match (self, other) {
            (Setting { id: a, .. }, Setting { id: b, .. }) => a == b,
            (DpiStages { .. }, DpiStages { .. }) => true,
            (ActiveStage(_), ActiveStage(_)) => true,
            (Button { slot: a, .. }, Button { slot: b, .. }) => a == b,
            (Macro { slot: a, .. }, Macro { slot: b, .. }) => a == b,
            (Lighting { area: a, .. }, Lighting { area: b, .. }) => a == b,
            (
                KeyMap {
                    layer: la, key: ka, ..
                },
                KeyMap {
                    layer: lb, key: kb, ..
                },
            ) => la == lb && ka == kb,
            (KeyPerf { keys: a, .. }, KeyPerf { keys: b, .. }) => a == b,
            (
                Advanced {
                    key: a, layer: la, ..
                },
                Advanced {
                    key: b, layer: lb, ..
                },
            ) => a == b && la == lb,
            (ProfileName { index: a, .. }, ProfileName { index: b, .. }) => a == b,
            (Peq { band: a, .. }, Peq { band: b, .. }) => a == b,
            (PeqAll { .. }, PeqAll { .. }) => true,
            (Peq { .. }, PeqAll { .. }) | (PeqAll { .. }, Peq { .. }) => true,
            (Adaptive(_), Adaptive(_)) => true,
            _ => false,
        }
    }
}
