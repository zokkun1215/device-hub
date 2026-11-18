//! Device HUB — GPUI フロントエンド (Liquid Glass)。
//!
//! - `app` … 窓の殻 (Shell): 状態・ルーティング・サイドバー・イベント配線
//! - `pages/*` … 各画面 (`impl Shell`)
//! - `overlay` … ポップアップ・モーダル・トースト
//! - `ui` … 部品 (カード・行・トグル・スライダー・セグメント…)
//! - `glass` … Liquid Glass の面 (GPU backdrop-filter)
//! - `motion` … ばねアニメーション・要素位置の記録
//! - `store` … バックエンド (devhub-core 契約) との橋。デバイスごとのワーカースレッド
//! - `theme` / `wallpaper` / `prefs` / `keycode`

pub mod app;
pub mod audio;
pub mod autostart;
pub mod cursor;
pub mod fwstate;
pub mod glass;
pub mod keycode;
pub mod livefx;
pub mod motion;
pub mod overlay;
pub mod pages;
pub mod palette;
pub mod prefs;
pub mod presets;
pub mod procmon;
pub use devhub_obs::rawinput;
pub mod store;
pub mod theme;
pub mod ui;
pub mod wallpaper;
