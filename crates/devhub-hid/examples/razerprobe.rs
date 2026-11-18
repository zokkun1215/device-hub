//! Razer 90B feature-report プローブ (読み取り専用)。
//!
//! DeathAdder V2 X HyperSpeed (1532:009C, ドングル) の全 HID インターフェースを開き、
//! OpenRazer (GPL-2.0, driver/razermouse_driver.c) が使うコマンド一式を試す。
//! - レポート: [status][txn=0x1F][remaining BE16][proto][size][class][id][args×80][crc][rsvd]
//! - CRC: bytes 2..=87 の XOR
//! - 経路: feature report (SET_REPORT / GET_REPORT, report ID 0x00, 91B)
//! 使い方: `cargo run -p devhub-hid --release --example razerprobe [-- --full]`

use hidapi::HidApi;

const VID: u16 = 0x1532;
const PID: u16 = 0x009C;

fn crc90(r: &[u8]) -> u8 {
    r[2..88].iter().fold(0u8, |a, b| a ^ b)
}

/// (class, id, size, args)
fn report(class: u8, id: u8, size: u8, args: &[u8]) -> [u8; 90] {
    let mut r = [0u8; 90];
    r[1] = 0x1F; // transaction id (DeathAdder V2 X HyperSpeed)
    r[5] = size;
    r[6] = class;
    r[7] = id;
    r[8..8 + args.len().min(80)].copy_from_slice(&args[..args.len().min(80)]);
    r[88] = crc90(&r);
    r
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 1 コマンド往復。応答全体 (report ID 込み) を返す。
fn roundtrip(dev: &hidapi::HidDevice, class: u8, id: u8, size: u8, args: &[u8]) -> Result<Vec<u8>, String> {
    let r = report(class, id, size, args);
    let mut buf = [0u8; 91];
    buf[1..].copy_from_slice(&r);
    dev.send_feature_report(&buf).map_err(|e| format!("send: {e}"))?;
    // Linux (OpenRazer) は受信待ち 31ms を入れている
    std::thread::sleep(std::time::Duration::from_millis(35));
    let mut rbuf = [0u8; 91];
    let n = dev
        .get_feature_report(&mut rbuf)
        .map_err(|e| format!("recv: {e}"))?;
    Ok(rbuf[..n].to_vec())
}

/// 応答の解釈。要求と class/id が一致し status が OK/BUSY のときだけ Some。
fn parse(resp: &[u8], class: u8, id: u8) {
    if resp.len() < 9 {
        println!("    ! short response ({}B)", resp.len());
        return;
    }
    let status = resp[1];
    let rclass = resp[7];
    let rid = resp[8];
    let size = resp[6] as usize;
    let args = &resp[9..(9 + size.min(resp.len() - 9)).max(0)];
    println!(
        "    status={status:02x} class={rclass:02x} id={rid:02x} size={size:02x} args={}",
        hex(args)
    );
    if rclass != class || rid != id {
        println!("    ! echo mismatch (expected class={class:02x} id={id:02x}) — 前回応答 or 無関係IFの可能性");
    }
}

fn main() {
    let full = std::env::args().any(|a| a == "--full");
    // 覚醒待ちリトライ回数 (1.5s 間隔)
    let retries: usize = std::env::args()
        .find_map(|a| a.strip_prefix("--retries=").and_then(|v| v.parse().ok()))
        .unwrap_or(30);
    let api = HidApi::new().unwrap();
    let devs: Vec<_> = api
        .device_list()
        .filter(|d| d.vendor_id() == VID && d.product_id() == PID)
        .collect();
    println!("== Razer 1532:{PID:04x} — {} interface(s) ==", devs.len());
    for d in &devs {
        println!(
            "\n-- if={} up={:04x} u={:02x} {:?}",
            d.interface_number(),
            d.usage_page(),
            d.usage(),
            d.product_string()
        );
        let Ok(dev) = api.open_path(d.path()) else {
            println!("    ! open failed");
            continue;
        };
        // この IF が 90B feature report を受け付けるか (1 回試す)
        let r = report(0x00, 0x81, 0x02, &[]);
        let mut buf = [0u8; 91];
        buf[1..].copy_from_slice(&r);
        if dev.send_feature_report(&buf).is_err() {
            println!("    (feature report なし — skip)");
            continue;
        };
        println!("    feature report OK — コントロール経路。マウスが休眠中なら動かしてください");
        // firmware 応答 (status 0x02) が返るまでリトライ
        let mut ok = false;
        for attempt in 1..=retries.max(1) {
            match roundtrip(&dev, 0x00, 0x81, 0x02, &[]) {
                Err(e) => println!("    ! firmware #{attempt}: {e}"),
                Ok(r) => {
                    let status = r.get(1).copied().unwrap_or(0xff);
                    println!(
                        "    firmware #{attempt}: status={status:02x} {}",
                        hex(&r)
                    );
                    if status == 0x02 {
                        ok = true;
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        if !ok {
            println!("    ! 応答なし (休眠継続 or 経路の問題) — この IF はスキップ");
            continue;
        }
        println!("    == マウス応答あり! ==");
        if !full {
            continue;
        }
        // 2) 残りの読み取り専用コマンド一式
        for (label, class, id, size, args) in [
            ("serial", 0x00u8, 0x82u8, 0x16u8, &[][..]),
            ("device_type", 0x00, 0x02, 0x02, &[]),
            ("dpi_stages", 0x04, 0x86, 0x26, &[0x01]),
            ("dpi_xy", 0x04, 0x85, 0x07, &[0x01]),
            ("polling", 0x00, 0x85, 0x01, &[]),
            ("battery", 0x07, 0x80, 0x02, &[]),
            ("charging", 0x07, 0x84, 0x02, &[]),
            ("idle_time", 0x07, 0x83, 0x02, &[]),
            ("low_batt", 0x07, 0x81, 0x01, &[]),
        ] {
            match roundtrip(&dev, class, id, size, args) {
                Err(e) => println!("    ! {label}: {e}"),
                Ok(resp) => {
                    println!("    {label} ({class:02x}/{id:02x}): {}", hex(&resp));
                    parse(&resp, class, id);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        break; // 制御 IF は 1 つで十分
    }
}

