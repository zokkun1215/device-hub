//! 常時起動: Windows ログオン時の自動起動登録 (`HKCU\...\Run`)。
//!
//! - **ドライバやサービスは作らない**。ユーザーの Run キーにこの exe のパスを
//!   1 つ書くだけ (管理者権限不要・Synapse のようなバックグラウンドサービスや
//!   常駐プロセスを追加しない)。
//! - アプリ側の設定 (「アプリ設定 → 常時起動」) からトグルする。オフにすると値を消す。
//! - exe を移動した場合はパスが変わるので、起動時に自己チェックして修正する
//!   (`Shell` の初期化から `sync()` を呼ぶ)。

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "DeviceHub";

#[cfg(windows)]
mod imp {
    use super::{RUN_KEY, VALUE_NAME};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_WRITE, REG_EXPAND_SZ, REG_SZ, RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Run キーに登録済みのコマンド文字列。
    pub fn command() -> Option<String> {
        let subkey = wide(RUN_KEY);
        let name = wide(VALUE_NAME);
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let mut ty = 0u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                &mut ty,
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if r != ERROR_SUCCESS || len < 2 {
            return None;
        }
        let n = ((len / 2).saturating_sub(1) as usize).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]))
    }

    /// 登録 (`Some`) / 削除 (`None`)。削除対象が無いのは成功とみなす。
    pub fn set(cmd: Option<&str>) -> Result<(), String> {
        let subkey = wide(RUN_KEY);
        let name = wide(VALUE_NAME);
        let mut h: HKEY = std::ptr::null_mut();
        let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut h) };
        if r != ERROR_SUCCESS {
            return Err(format!("Run キーを開けません ({r})"));
        }
        let result = match cmd {
            Some(c) => {
                let mut v = wide(c);
                let ty = if c.contains('%') { REG_EXPAND_SZ } else { REG_SZ };
                unsafe {
                    RegSetValueExW(
                        h,
                        name.as_ptr(),
                        0,
                        ty,
                        v.as_mut_ptr().cast(),
                        (v.len() * 2) as u32,
                    )
                }
            }
            None => unsafe { RegDeleteValueW(h, name.as_ptr()) },
        };
        unsafe { RegCloseKey(h) };
        if result == ERROR_SUCCESS || cmd.is_none() {
            Ok(())
        } else {
            Err(format!("レジストリ書き込みに失敗 ({result})"))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn command() -> Option<String> {
        None
    }
    pub fn set(_cmd: Option<&str>) -> Result<(), String> {
        Err("Windows 以外では自動起動登録はできません".into())
    }
}

/// 現在の exe パス (引用符付きコマンド)。
fn self_command() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| format!("\"{}\"", p.display()))
}

/// Run キーに登録済みか (値があれば true)。
pub fn enabled() -> bool {
    imp::command().is_some()
}

/// 登録/解除。`on=false` で値を削除する。
pub fn set_enabled(on: bool) -> Result<(), String> {
    if on {
        let cmd =
            self_command().ok_or_else(|| "実行ファイルのパスが取れません".to_string())?;
        imp::set(Some(&cmd))
    } else {
        imp::set(None)
    }
}

/// 起動時の自己チェック: 登録はあるがパスが古い (exe を移した) 場合だけ書き直す。
/// 戻り値 = 現在も登録が有効か。
pub fn sync() -> bool {
    let Some(cur) = self_command() else {
        return false;
    };
    match imp::command() {
        Some(existing) => {
            if existing != cur {
                let _ = imp::set(Some(&cur));
            }
            true
        }
        None => false,
    }
}
