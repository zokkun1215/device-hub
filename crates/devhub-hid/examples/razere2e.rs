//! Razer ドライバ経路の実機検証 — razer::open → caps → read → apply(読み戻し) → 復元。
//! `cargo run -p devhub-hid --example razere2e`
//!
//! マウスが休眠している場合は覚醒を待つ (マウスを動かしてください)。

use devhub_core::backend::DeviceDriver;
use devhub_core::model::*;
use devhub_hid::razer;
use devhub_hid::transport;

fn main() {
    let api = transport::open().expect("hidapi");
    // 制御チャネルは if0 (0001/02) — if1 の同名 usage コレクションには
    // feature report が無いので interface で区別する。
    let info_raw = api
        .device_list()
        .find(|d| {
            d.vendor_id() == 0x1532
                && d.product_id() == 0x009C
                && d.interface_number() == 0
                && d.usage_page() == 0x0001
                && d.usage() == 0x02
        })
        .expect("DeathAdder V2 X HyperSpeed (1532:009C if0)");
    let dev = info_raw.open_device(&api).expect("open");
    let info = DeviceInfo {
        id: DeviceId("hid:1532:009c".into()),
        kind: DeviceKind::Mouse,
        name: "DeathAdder V2 X HyperSpeed".into(),
        vendor: "Razer".into(),
        model: "deathadder-v2-x-hyperspeed".into(),
        vid: 0x1532,
        pid: 0x009C,
        transport: Transport::Wireless24G,
        firmware: None,
        dongle_firmware: None,
        serial: None,
        driver: DriverKind::Razer,
        supported: true,
        asleep: false,
        battery: None,
        board_id: None,
    };
    let d = razer::open(dev, info, None).expect("driver");

    // 覚醒待ち: info() は休眠中 fw を取れない。
    println!("== 接続待ち (マウスが休眠中なら動かしてください)");
    let mut i = d.info();
    for n in 1..=120 {
        if i.firmware.is_some() {
            break;
        }
        if n % 10 == 0 {
            println!("   ...まだ応答なし (#{n})");
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
        i = d.info();
    }
    println!(
        "== info: fw={:?} serial={:?} battery={:?}",
        i.firmware, i.serial, i.battery
    );
    assert!(i.firmware.is_some(), "マウスが応答しません (休眠 or 未接続?)");

    let c = d.caps();
    println!("== caps: settings={:?}", c.settings.iter().map(|s| &s.id).collect::<Vec<_>>());
    println!(
        "   mouse: dpi {}..{} step {} stages {}..{}",
        c.mouse.as_ref().unwrap().dpi_min,
        c.mouse.as_ref().unwrap().dpi_max,
        c.mouse.as_ref().unwrap().dpi_step,
        c.mouse.as_ref().unwrap().min_stages,
        c.mouse.as_ref().unwrap().max_stages
    );

    let s = d.read().expect("read");
    let ms = s.mouse.as_ref().unwrap();
    let orig_polling = s.settings.get("polling_wireless").and_then(|v| match v {
        SettingValue::Choice(c) => Some(*c),
        _ => None,
    });
    let orig_sleep = s.settings.get("sleep_min").cloned();
    let orig_low = s.settings.get("low_battery_threshold").cloned();
    let orig_stages = ms.stages.clone();
    let orig_active = ms.active_stage;
    println!(
        "== read: polling={orig_polling:?} sleep={orig_sleep:?} low_batt={orig_low:?} battery={:?}",
        d.battery().ok()
    );
    for (k, st) in ms.stages.iter().enumerate() {
        println!("   stage {k}: {}x{}", st.x, st.y.unwrap_or(st.x));
    }
    println!("   active stage = {orig_active}");

    let mut pass = 0;
    let mut fail = 0;
    let mut checks: Vec<(&str, bool)> = vec![];

    // ---- 1) ポーリングレート往復 (500Hz → 元に戻す)
    println!("\n== 1) ポーリングレート往復");
    let target = if orig_polling == Some(500) { 125 } else { 500 };
    let mut ok = true;
    ok &= d
        .apply(&Change::Setting {
            id: "polling_wireless".into(),
            value: SettingValue::Choice(target),
        })
        .is_ok();
    let back = d
        .read()
        .ok()
        .and_then(|s| s.settings.get("polling_wireless").cloned());
    println!("   set {target} -> read {back:?}");
    ok &= back == Some(SettingValue::Choice(target));
    if let Some(orig) = orig_polling {
        ok &= d
            .apply(&Change::Setting {
                id: "polling_wireless".into(),
                value: SettingValue::Choice(orig),
            })
            .is_ok();
        let back = d
            .read()
            .ok()
            .and_then(|s| s.settings.get("polling_wireless").cloned());
        println!("   restore {orig} -> read {back:?}");
        ok &= back == Some(SettingValue::Choice(orig));
    }
    checks.push(("polling roundtrip", ok));

    // ---- 2) スリープ時間往復
    println!("\n== 2) スリープ時間往復");
    let target = match orig_sleep {
        Some(SettingValue::Choice(10)) => 15,
        _ => 10,
    };
    let mut ok = true;
    ok &= d
        .apply(&Change::Setting {
            id: "sleep_min".into(),
            value: SettingValue::Choice(target),
        })
        .is_ok();
    let back = d.read().ok().and_then(|s| s.settings.get("sleep_min").cloned());
    println!("   set {target}分 -> read {back:?}");
    ok &= back == Some(SettingValue::Choice(target));
    if let Some(orig) = orig_sleep {
        ok &= d
            .apply(&Change::Setting {
                id: "sleep_min".into(),
                value: orig.clone(),
            })
            .is_ok();
        let back = d.read().ok().and_then(|s| s.settings.get("sleep_min").cloned());
        println!("   restore -> read {back:?}");
        ok &= back == Some(orig);
    }
    checks.push(("sleep roundtrip", ok));

    // ---- 3) 低バッテリー閾値往復
    println!("\n== 3) 低バッテリー閾値往復");
    let target = match orig_low {
        Some(SettingValue::Choice(15)) => 20,
        _ => 15,
    };
    let mut ok = true;
    ok &= d
        .apply(&Change::Setting {
            id: "low_battery_threshold".into(),
            value: SettingValue::Choice(target),
        })
        .is_ok();
    let back = d
        .read()
        .ok()
        .and_then(|s| s.settings.get("low_battery_threshold").cloned());
    println!("   set {target}% -> read {back:?}");
    ok &= back == Some(SettingValue::Choice(target));
    if let Some(orig) = orig_low {
        ok &= d
            .apply(&Change::Setting {
                id: "low_battery_threshold".into(),
                value: orig.clone(),
            })
            .is_ok();
        let back = d
            .read()
            .ok()
            .and_then(|s| s.settings.get("low_battery_threshold").cloned());
        println!("   restore -> read {back:?}");
        ok &= back == Some(orig);
    }
    checks.push(("low battery roundtrip", ok));

    // ---- 4) DPI ステージ同一内容の書き戻し (値が壊れないこと)
    println!("\n== 4) DPI ステージ書き戻し (同一内容)");
    let mut ok = d
        .apply(&Change::DpiStages {
            stages: orig_stages.clone(),
            active: orig_active,
        })
        .is_ok();
    let back = d.read().expect("read after write");
    let ms2 = back.mouse.as_ref().unwrap();
    for (k, (a, b)) in orig_stages.iter().zip(ms2.stages.iter()).enumerate() {
        if a.x != b.x || a.y != b.y {
            println!("   stage {k}: {} != {}", a.x, b.x);
            ok = false;
        }
    }
    ok &= ms2.stages.len() == orig_stages.len();
    println!("   active={} stages={:?}", ms2.active_stage, ms2.stages.iter().map(|s| s.x).collect::<Vec<_>>());
    checks.push(("dpi stages idempotent", ok));

    // ---- 5) 現在段の切替 (隣の段 → 元に戻す)
    println!("\n== 5) 現在段切替往復");
    let other = if orig_active + 1 < orig_stages.len() {
        orig_active + 1
    } else {
        orig_active.saturating_sub(1)
    };
    let mut ok = true;
    ok &= d.apply(&Change::ActiveStage(other)).is_ok();
    let back = d.read().expect("read");
    let ms3 = back.mouse.as_ref().unwrap();
    println!("   switch to {other} -> active={}", ms3.active_stage);
    ok &= ms3.active_stage == other;
    ok &= d.apply(&Change::ActiveStage(orig_active)).is_ok();
    let back = d.read().expect("read");
    let ms4 = back.mouse.as_ref().unwrap();
    println!("   restore {orig_active} -> active={}", ms4.active_stage);
    ok &= ms4.active_stage == orig_active;
    checks.push(("active stage roundtrip", ok));

    // ---- 6) 電池
    println!("\n== 6) 電池");
    let bat = d.battery();
    println!("   battery = {bat:?}");
    checks.push(("battery", bat.is_ok()));

    for (name, ok) in &checks {
        println!("{}: {name}", if *ok { "PASS" } else { "FAIL" });
        if *ok {
            pass += 1;
        } else {
            fail += 1;
        }
    }
    println!("\n== {pass} PASS / {fail} FAIL");
    if fail > 0 {
        std::process::exit(1);
    }
}
