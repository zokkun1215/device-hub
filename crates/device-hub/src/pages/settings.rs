//! アプリ設定 (見た目・動作・バックエンド)。

use crate::app::{PopItem, Shell};
use crate::theme;
use crate::ui::{self, Btn};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::sync::Arc;

impl Shell {
    pub fn page_settings(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = theme::pal();
        let light = theme::is_light();

        let mut accents = div().flex().gap(px(10.));
        for (i, (name, hx)) in theme::ACCENTS.iter().enumerate() {
            let on = theme::accent_index() as usize == i;
            accents = accents.child(
                div()
                    .id(SharedString::from(format!("accent-{i}")))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .child(ui::swatch(theme::hex(*hx, 1.), 28., on))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(if on { p.fg } else { p.faint })
                            .child(*name),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        theme::set_accent(i as u8);
                        this.prefs.accent = i as u8;
                        this.prefs.save();
                        cx.notify();
                    })),
            );
        }

        let wall = self.prefs.wallpaper.clone();
        let look = ui::card_titled("見た目", "palette", None).child(
            ui::body()
                .pb(px(12.))
                .child(ui::row_first(
                    "テーマ",
                    None,
                    ui::segmented(
                        "theme",
                        &["ダーク".into(), "ライト".into()],
                        light as usize,
                        cx,
                        |this, i, cx| this.set_light(i == 1, cx),
                    )
                    .w(px(200.)),
                ))
                .child(ui::row("アクセントカラー", None, accents))
                .child(ui::row(
                    "フォント",
                    Some("Geist / IBM Plex Sans JP / Zen Maru Gothic は同梱。他は未インストール時にフォールバック"),
                    ui::select(
                        "font",
                        theme::FONT_OPTIONS
                            .iter()
                            .enumerate()
                            .map(|(i, o)| PopItem::new(o.1, i == theme::font_index()))
                            .collect(),
                        theme::FONT_OPTIONS[theme::font_index()].1,
                        300.,
                        cx,
                        |this, i, _w, cx| {
                            let id = theme::FONT_OPTIONS[i].0;
                            theme::set_font_id(id);
                            // Zen Maru 系を初めて選んだときここで登録する。
                            theme::install_fonts(cx);
                            this.prefs.font = id.into();
                            this.prefs.save();
                            cx.notify();
                        },
                    ),
                ))
                .child(
                    div().px(px(14.)).pb(px(12.)).child(
                        div()
                            .font(theme::ui_font())
                            .text_size(px(14.))
                            .text_color(p.sub)
                            .child("プレビュー: あいうえお ABC 1600 DPI / 8KHz"),
                    ),
                )
                .child(ui::row(
                    "Liquid Glass",
                    Some("オフにすると不透明な面になり、GPU 負荷がさらに下がります"),
                    ui::toggle("glass", theme::glass_on(), cx, |this, on, cx| {
                        theme::set_glass(on);
                        this.prefs.glass = on;
                        this.prefs.save();
                        cx.notify();
                    }),
                ))
                .child(ui::row(
                    "アニメーション",
                    None,
                    ui::toggle("motion", theme::motion_on(), cx, |this, on, cx| {
                        theme::set_motion(on);
                        this.prefs.motion = on;
                        this.prefs.save();
                        cx.notify();
                    }),
                ))
                .child(ui::row(
                    "壁紙",
                    Some(match &wall {
                        Some(w) => w.as_str(),
                        None => "オーロラ (自動生成)",
                    }),
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(ui::button(
                            "wall-pick",
                            "画像を選ぶ",
                            Some("image"),
                            Btn::Glass,
                            cx,
                            |this, _, cx| {
                                if let Some(f) = rfd::FileDialog::new()
                                    .add_filter("画像", &["png", "jpg", "jpeg"])
                                    .pick_file()
                                {
                                    this.prefs.wallpaper = Some(f.display().to_string());
                                    this.prefs.save();
                                    cx.notify();
                                }
                            },
                        ))
                        .when(wall.is_some(), |d| {
                            d.child(ui::button(
                                "wall-reset",
                                "既定",
                                None,
                                Btn::Ghost,
                                cx,
                                |this, _, cx| {
                                    this.prefs.wallpaper = None;
                                    this.prefs.save();
                                    cx.notify();
                                },
                            ))
                        }),
                )),
        );

        let demo = self.hub.demo;
        let backend = ui::card_titled("デバイス接続", "usb", None).child(
            ui::body()
                .pb(px(12.))
                .child(ui::row_first(
                    "モード",
                    Some(if demo {
                        "デモデバイスを表示中 (実機には書き込みません)"
                    } else {
                        "接続された実機を使用中"
                    }),
                    ui::segmented(
                        "backend",
                        &["実機".into(), "デモ".into()],
                        demo as usize,
                        cx,
                        |this, i, cx| {
                            if i == 1 {
                                this.hub
                                    .switch_backend(Arc::new(devhub_core::demo::DemoHub::new()));
                            } else {
                                match devhub_hid::connect() {
                                    Ok(b) => this.hub.switch_backend(b),
                                    Err(e) => {
                                        this.toast(format!("HID を開けません: {e}"), true, cx)
                                    }
                                }
                            }
                            this.route = crate::app::Route::Settings;
                            cx.notify();
                        },
                    )
                    .w(px(200.)),
                ))
                .child(ui::row(
                    "再検索",
                    Some("2 秒ごとに自動で検索しています"),
                    ui::button(
                        "rescan2",
                        "今すぐ検索",
                        Some("refresh-cw"),
                        Btn::Glass,
                        cx,
                        |this, _, cx| {
                            this.hub.scan();
                            cx.notify();
                        },
                    ),
                )),
        );

        let fw = ui::card_titled("ファームウェア更新", "download", None).child(
            ui::body()
                .pb(px(12.))
                .child(ui::row_first(
                    "更新を確認する",
                    Some(
                        "起動時に公式の配布元を確認します (24 時間に 1 回まで・書き込みはしません)",
                    ),
                    ui::toggle("fw-check", self.prefs.fw_check, cx, |this, on, cx| {
                        this.prefs.fw_check = on;
                        this.prefs.save();
                        this.hub.fw_enabled = on;
                        cx.notify();
                    }),
                ))
                .child(ui::row(
                    "Windows 通知にも出す",
                    Some("新しいファームウェアが見つかったら通知センターにも表示"),
                    ui::toggle(
                        "fw-native",
                        self.prefs.fw_native_notify,
                        cx,
                        |this, on, cx| {
                            this.prefs.fw_native_notify = on;
                            this.prefs.save();
                            this.hub.fw_native = on;
                            cx.notify();
                        },
                    ),
                ))
                .child(ui::row(
                    "今すぐ確認",
                    Some("接続中のデバイス全部を確認します"),
                    ui::button(
                        "fw-now",
                        "確認",
                        Some("refresh-cw"),
                        Btn::Glass,
                        cx,
                        |this, _, cx| {
                            let n = this.hub.check_all_firmware(true);
                            let msg = if n == 0 {
                                "確認できるデバイスがありません (対応デバイスを開いてください)"
                                    .to_string()
                            } else {
                                format!("{n} 台のファームウェアを確認しています…")
                            };
                            this.toast(msg, false, cx);
                        },
                    ),
                )),
        );

        let autostart = ui::card_titled("常時起動", "power", None).child(
            ui::body()
                .pb(px(12.))
                .child(ui::row_first(
                    "Windows 起動時に自動で立ち上げる",
                    Some(
                        "Synapse の代わりにこのアプリをログオン時に起動します。\
                         プロセス自動適用・バッテリー表示・休眠検知を使うには常時起動が便利です",
                    ),
                    ui::toggle("autostart", self.prefs.autostart, cx, |this, on, cx| {
                        match crate::autostart::set_enabled(on) {
                            Ok(()) => {
                                this.prefs.autostart = on;
                                this.prefs.save();
                                if on {
                                    this.toast("自動起動を登録しました", false, cx);
                                }
                            }
                            Err(e) => {
                                this.toast(format!("自動起動の登録に失敗: {e}"), true, cx);
                            }
                        }
                        cx.notify();
                    }),
                ))
                .child(ui::row(
                    "Razer Synapse を止める",
                    Some(
                        "Synapse が動いていると同じ HID を取り合います。置き換えるときは\
                         タスクトレイの Razer Central を終了し、スタートアップ登録を外してください",
                    ),
                    ui::button(
                        "kill-razer",
                        "Synapse を終了",
                        Some("x-circle"),
                        Btn::Glass,
                        cx,
                        |this, _, cx| {
                            // UI スレッドを止めないため、結果を待たずに投げる。
                            let ps = "Get-Process | Where-Object { $_.Name -match '^Razer|^RzEngine|^razerwdl' } | Stop-Process -Force -ErrorAction SilentlyContinue";
                            let spawned = std::process::Command::new("powershell")
                                .args(["-NoProfile", "-Command", ps])
                                .spawn()
                                .is_ok();
                            this.toast(
                                if spawned {
                                    "Razer プロセスの停止を試みています…".to_string()
                                } else {
                                    "PowerShell を起動できませんでした".to_string()
                                },
                                !spawned,
                                cx,
                            );
                        },
                    ),
                )),
        );

        let about = ui::card_titled("このアプリについて", "info", None).child(
            ui::body()
                .pb(px(14.))
                .gap(px(6.))
                .pt(px(4.))
                .child(ui::sub(format!("Device Hub {}", env!("CARGO_PKG_VERSION"))))
                .child(ui::faint("mouse.fit / hub.xsyd.top の設定をブラウザなしで行うための個人用ツール。GPUI + Liquid Glass。"))
                .child(ui::faint("新しいデバイスの追加方法: docs/ADD_DEVICE.md")),
        );

        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme::PAGE_GAP))
                    .p(px(theme::PAGE_PAD))
                    .pt(px(theme::PAGE_TOP))
                    .pb(px(theme::PAGE_BOT))
                    .child(ui::title("アプリ設定"))
                    .child(ui::grid2(vec![
                        look.into_any_element(),
                        backend.into_any_element(),
                        autostart.into_any_element(),
                        fw.into_any_element(),
                        about.into_any_element(),
                    ])),
            )
    }
}
