use hidapi::HidApi;
fn main() {
    let api = HidApi::new().unwrap();
    for d in api.device_list() {
        let all = std::env::args().any(|a| a == "--all");
        if all || d.vendor_id() == 0x33e4 || d.vendor_id() == 0x1ca6 || d.vendor_id() == 0x1532 {
            println!(
                "{:04x}:{:04x} if={} up={:04x} u={:02x} {:?}",
                d.vendor_id(),
                d.product_id(),
                d.interface_number(),
                d.usage_page(),
                d.usage(),
                d.product_string()
            );
        }
    }
    println!("-- total {} --", api.device_list().count());
}
