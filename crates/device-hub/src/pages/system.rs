//! 「一般」タブ: デバイス情報・プロファイル・バックアップ・その他の設定とメンテナンス。

use crate::app::{Modal, Shell, Tab};
use crate::theme;
use crate::ui::{self, Btn};
use devhub_core::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::rc::Rc;

impl Shell {
    pub fn page_system(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::pal();
        let Some(s) = self.cur() else {
            return div().into_any_element();
        };
        let info = s.info.clone();
        let caps = s.caps.clone().unwrap_or_else(Capabilities::empty);
        let state = s.state.clone();
        let did = s.id.clone();

        // ---- 情報
        let kv = |k: &str, v: String| {
            div()
                .flex()
                .py(px(7.))
                .border_t_1()
                .border_color(p.hair)
                .child(
                    div()
                        .w(px(150.))
                        .text_size(px(12.5))
                        .text_color(p.sub)
                        .child(k.to_string()),
                )
                .child(div().flex_1().text_size(px(12.5)).text_color(p.fg).child(v))
        };
        let info_card = ui::card_titled("デバイス情報", "info", None).child(
            ui::body()
                .pb(px(14.))
                .child(kv("製品", format!("{} {}", info.vendor, info.name)))
                .child(kv("VID:PID", format!("{:04X}:{:04X}", info.vid, info.pid)))
                .child(kv(
                    "ファームウェア",
                    info.firmware.clone().unwrap_or_else(|| "-".into()),
                ))
                .when_some(info.dongle_firmware.clone(), |d, f| {
                    d.child(kv("レシーバー", f))
                })
                .when_some(info.serial.clone(), |d, f| d.child(kv("シリアル", f)))
                .child(kv(
                    "ドライバ",
                    match info.driver {
                        DriverKind::Xsyd => "XSYD (hub.xsyd.top 互換)".into(),
                        DriverKind::Mousefit => "mouse.fit 互換".into(),
                        DriverKind::Walkplay => "WalkPlay (peq.szwalkplay.com 互換)".into(),
                        DriverKind::Atk => "ATK HUB 互換".into(),
                        DriverKind::Lamzu => "LAMZU Aurora 互換".into(),
                        DriverKind::Razer => "Razer (Synapse 置換)".into(),
                        DriverKind::Unsupported => "未対応".into(),
                    },
                ))
                .child(kv("ID", info.id.0.clone())),
        );

        let model = info.model.clone();
        let presets = self.presets_card(&model, cx);
        let mut cards: Vec<AnyElement> = vec![info_card.into_any_element(), presets];

        // ---- ファームウェア更新 (あれば最上段へ)
        if let Some(up) = self
            .hub
            .fw_status
            .get(&did)
            .filter(|u| u.new_available)
            .cloned()
        {
            let mut line = "新しいファームウェアが公開されています".to_string();
            if let Some(v) = &up.version {
                line = format!("{line} ({v})");
            }
            let ago = fmt_ago(up.checked_at);
            let mut body = ui::body()
                .pb(px(14.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.fg)
                        .pb(px(6.))
                        .child(line),
                )
                .child(ui::faint(format!("最終確認: {ago}")));
            let mut row = div().flex().items_center().gap(px(8.)).pt(px(10.));
            if let Some(url) = up.url.clone() {
                row = row.child(ui::button(
                    "fw-open",
                    "公式ページを開く",
                    Some("download"),
                    Btn::Primary,
                    cx,
                    move |_, _, cx| cx.open_url(&url),
                ));
            }
            row = row.child(ui::button(
                "fw-dismiss",
                "閉じる",
                Some("x"),
                Btn::Glass,
                cx,
                move |this, _, cx| {
                    this.hub.fw_dismiss(&did);
                    cx.notify();
                },
            ));
            body = body.child(row);
            cards.insert(
                0,
                ui::card_titled("ファームウェア更新", "info", None)
                    .child(body)
                    .into_any_element(),
            );
        }

        // ---- プロファイル
        if let (Some(pc), Some(st)) = (caps.profiles.clone(), state.as_ref()) {
            if pc.count > 1 {
                let mut body = ui::body().pb(px(14.));
                for i in 0..pc.count {
                    let active = st.profile.active == i;
                    let name = st
                        .profile
                        .names
                        .get(i as usize)
                        .filter(|n| !n.is_empty())
                        .cloned();
                    let shown = name
                        .clone()
                        .unwrap_or_else(|| format!("プロファイル {}", i + 1));
                    let mut row = div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .py(px(8.))
                        .when(i > 0, |d| d.border_t_1().border_color(p.hair))
                        .child(
                            div()
                                .size(px(28.))
                                .rounded(px(9.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if active { p.accent } else { p.ctrl })
                                .text_size(px(11.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if active { p.on_accent } else { p.sub })
                                .child(format!("P{}", i + 1)),
                        )
                        .child(div().flex_1().text_size(px(13.)).child(shown.clone()))
                        .when(active, |d| d.child(ui::chip("使用中", p.ok)));
                    if !active {
                        row = row.child(ui::button(
                            format!("prof-use-{i}"),
                            "切り替え",
                            None,
                            Btn::Glass,
                            cx,
                            move |this, _, cx| {
                                this.apply(Change::Profile(i), cx);
                                if let Some(id) = this.device_id() {
                                    this.hub.reload(&id);
                                }
                            },
                        ));
                    }
                    if pc.renamable {
                        row = row.child(ui::icon_button(
                            format!("prof-ren-{i}"),
                            "pencil",
                            cx,
                            move |this, w, cx| {
                                this.modal = Some(Modal::Text {
                                    title: format!("P{} の名前", i + 1),
                                    on_ok: Rc::new(move |this, v, _, cx| {
                                        this.apply(
                                            Change::ProfileName {
                                                index: i,
                                                name: v.trim().to_string(),
                                            },
                                            cx,
                                        )
                                    }),
                                });
                                this.input("modal", "", "", w, cx);
                                let cur = name.clone().unwrap_or_default();
                                this.set_input("modal", &cur, w, cx);
                                cx.notify();
                            },
                        ));
                    }
                    body = body.child(row);
                }
                cards.push(
                    ui::card_titled("オンボードプロファイル", "layers", None)
                        .child(body)
                        .into_any_element(),
                );
            }
        }

        // ---- バックアップ
        let name = info.name.clone();
        let backup = ui::card_titled("バックアップ", "hard-drive-download", None).child(
            ui::body()
                .pb(px(12.))
                .child(ui::row_first(
                    "設定を書き出す",
                    Some("現在の全設定を JSON ファイルに保存"),
                    ui::button(
                        "export",
                        "書き出し",
                        Some("download"),
                        Btn::Glass,
                        cx,
                        move |this, _, cx| {
                            let Some(st) = this.cur().and_then(|s| s.state.clone()) else {
                                return;
                            };
                            let file = rfd::FileDialog::new()
                                .set_file_name(format!("{}.devhub.json", name.replace(' ', "_")))
                                .add_filter("Device Hub", &["json"])
                                .save_file();
                            if let Some(f) = file {
                                match serde_json::to_string_pretty(&st)
                                    .map(|s| std::fs::write(&f, s))
                                {
                                    Ok(Ok(())) => this.toast(
                                        format!("保存しました: {}", f.display()),
                                        false,
                                        cx,
                                    ),
                                    Ok(Err(e)) => this.toast(format!("保存に失敗: {e}"), true, cx),
                                    Err(e) => this.toast(format!("保存に失敗: {e}"), true, cx),
                                }
                            }
                        },
                    ),
                ))
                .child(ui::row(
                    "設定を読み込む",
                    Some("書き出した JSON をこのデバイスへ書き込む"),
                    ui::button(
                        "import",
                        "読み込み",
                        Some("upload"),
                        Btn::Glass,
                        cx,
                        |this, _, cx| {
                            let Some(f) = rfd::FileDialog::new()
                                .add_filter("Device Hub", &["json"])
                                .pick_file()
                            else {
                                return;
                            };
                            let parsed = std::fs::read_to_string(&f)
                                .map_err(|e| e.to_string())
                                .and_then(|s| {
                                    serde_json::from_str::<DeviceState>(&s)
                                        .map_err(|e| e.to_string())
                                });
                            match parsed {
                                Ok(mut st) => {
                                    // 別機種・非対応のファイルはここで弾く。
                                    let compat = this
                                        .device_id()
                                        .and_then(|id| this.hub.session(&id))
                                        .and_then(|s| {
                                            s.caps.clone().zip(s.state.clone())
                                        });
                                    let Some((caps, cur)) = compat else {
                                        this.toast(
                                            "デバイスの設定を読み込めていません",
                                            true,
                                            cx,
                                        );
                                        return;
                                    };
                                    match crate::store::state_compatible(&caps, &st) {
                                        Err(e) => {
                                            this.toast(
                                                format!("読み込めません: {e}"),
                                                true,
                                                cx,
                                            );
                                        }
                                        Ok(warn) => {
                                            // ファイルは現在選択中の構成へだけ反映する。
                                            st.profile.active = cur.profile.active;
                                            let body = match &warn {
                                                Some(w) => format!(
                                                    "現在の構成をファイルの内容で上書きします ({w})。"
                                                ),
                                                None => "現在の構成をファイルの内容で上書きします。"
                                                    .to_string(),
                                            };
                                            this.confirm(
                                                "設定を読み込む",
                                                &body,
                                                "書き込む",
                                                true,
                                                Rc::new(move |this, _, cx| {
                                                    match this.apply_state(&st, true, cx) {
                                                        crate::app::RestoreOutcome::Applied(
                                                            n,
                                                        ) => this.toast(
                                                            format!("{n} 項目を書き込みました"),
                                                            false,
                                                            cx,
                                                        ),
                                                        crate::app::RestoreOutcome::Switching => {
                                                            this.toast(
                                                                "構成を切り替えて適用します…",
                                                                false,
                                                                cx,
                                                            )
                                                        }
                                                        crate::app::RestoreOutcome::Nothing => this
                                                            .toast(
                                                                "適用できませんでした",
                                                                true,
                                                                cx,
                                                            ),
                                                    }
                                                }),
                                                cx,
                                            );
                                        }
                                    }
                                }
                                Err(e) => this.toast(format!("読み込めません: {e}"), true, cx),
                            }
                        },
                    ),
                )),
        );
        cards.push(backup.into_any_element());

        cards.extend(self.section_cards(Tab::System, window, cx));
        ui::grid2(cards).into_any_element()
    }

    /// PC に保存するプリセットのカード。
    fn presets_card(&mut self, model: &str, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::pal();
        let list = crate::presets::list(model);
        let mut body = ui::body().pb(px(12.));
        if list.is_empty() {
            body = body.child(ui::faint("まだありません。ゲームごとの設定などを保存しておくと、ワンクリックで切り替えられます。").py(px(8.)));
        } else {
            body = body
                .child(ui::faint("プリセットは現在選択中の構成にだけ反映されます。").py(px(6.)));
        }
        for (i, pr) in list.iter().enumerate() {
            let (a, b, c) = (pr.clone(), pr.clone(), pr.clone());
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .py(px(7.))
                    .when(i > 0, |d| d.border_t_1().border_color(p.hair))
                    .child(ui::icon("bookmark", 15., p.accent))
                    .child(div().flex_1().text_size(px(13.)).child(pr.name.clone()))
                    .child(ui::button(
                        format!("pre-apply-{i}"),
                        "適用",
                        Some("check"),
                        Btn::Glass,
                        cx,
                        move |this, _, cx| match crate::presets::load(&a) {
                            Ok(st) => this.apply_preset(&a.name, st, cx),
                            Err(e) => this.toast(format!("読み込めません: {e}"), true, cx),
                        },
                    ))
                    .child(ui::icon_button(
                        format!("pre-ren-{i}"),
                        "pencil",
                        cx,
                        move |this, w, cx| {
                            let b = b.clone();
                            this.modal = Some(Modal::Text {
                                title: "プリセット名を変更".into(),
                                on_ok: Rc::new(move |this, v, _, cx| {
                                    if let Err(e) = crate::presets::rename(&b, v.trim()) {
                                        this.toast(e, true, cx);
                                    }
                                    cx.notify();
                                }),
                            });
                            this.input("modal", "", "", w, cx);
                            this.set_input("modal", &c.name, w, cx);
                            cx.notify();
                        },
                    ))
                    .child(ui::icon_button(format!("pre-del-{i}"), "trash", cx, {
                        let d = pr.clone();
                        move |this, _, cx| {
                            let d = d.clone();
                            this.confirm(
                                "プリセットを削除",
                                &format!(
                                    "「{}」を削除します (デバイスの設定は変わりません)。",
                                    d.name
                                ),
                                "削除",
                                true,
                                Rc::new(move |_this, _, cx| {
                                    let _ = crate::presets::delete(&d);
                                    cx.notify();
                                }),
                                cx,
                            );
                        }
                    })),
            );
        }
        let model2 = model.to_string();
        let save = ui::button(
            "pre-save",
            "現在の設定を保存",
            Some("plus"),
            Btn::Primary,
            cx,
            move |this, w, cx| {
                let model = model2.clone();
                this.modal = Some(Modal::Text {
                    title: "プリセットとして保存".into(),
                    on_ok: Rc::new(move |this, v, _, cx| {
                        let name = v.trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        let Some(st) = this.cur().and_then(|s| s.state.clone()) else {
                            return;
                        };
                        match crate::presets::save(&model, &name, &st) {
                            Ok(()) => this.toast(format!("「{name}」を保存しました"), false, cx),
                            Err(e) => this.toast(format!("保存できません: {e}"), true, cx),
                        }
                    }),
                });
                this.input("modal", "", "", w, cx);
                this.set_input("modal", "", w, cx);
                cx.notify();
            },
        );
        ui::card_titled(
            "プリセット (この PC に保存)",
            "bookmark",
            Some(save.into_any_element()),
        )
        .child(body)
        .into_any_element()
    }
}

/// 「最終確認」用の相対時刻。
fn fmt_ago(ts: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let d = now.saturating_sub(ts);
    if d < 60 {
        "たった今".into()
    } else if d < 3600 {
        format!("{} 分前", d / 60)
    } else if d < 86400 {
        format!("{} 時間前", d / 3600)
    } else {
        format!("{} 日前", d / 86400)
    }
}
