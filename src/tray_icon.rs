use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::UI::Shell::{
    ExtractIconExW, Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO,
    NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::native_interop::WM_APP_TRAY;

const APP_TRAY_ICON_ID: u32 = 1;
const LEGACY_CODEX_TRAY_ICON_ID: u32 = 2;
const LEGACY_ANTIGRAVITY_TRAY_ICON_ID: u32 = 3;

/// Menu item ID for toggling widget visibility (used by window.rs context menu).
pub const IDM_TOGGLE_WIDGET: u16 = 70;

/// Actions the tray message handler can request from the main window.
pub enum TrayAction {
    None,
    ToggleWidget,
    ShowContextMenu,
}

/// Identifies the provider that originated a notification. All notifications
/// now use the one application tray icon rather than provider-specific icons.
#[derive(Clone, Copy)]
pub enum TrayIconKind {
    Claude,
    Codex,
    Antigravity,
}

pub struct TrayIconData {
    pub tooltip: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeSeverity {
    Information,
    Warning,
}

/// Load the same embedded icon used by the executable and desktop shortcut.
pub fn create_icon() -> HICON {
    load_embedded_app_icon()
}

fn load_embedded_app_icon() -> HICON {
    unsafe {
        let mut exe_buf = [0u16; 260];
        let len = GetModuleFileNameW(None, &mut exe_buf) as usize;
        if len == 0 {
            return HICON::default();
        }

        let mut small_icon = HICON::default();
        let mut large_icon = HICON::default();
        let extracted = ExtractIconExW(
            PCWSTR::from_raw(exe_buf.as_ptr()),
            0,
            Some(&mut large_icon),
            Some(&mut small_icon),
            1,
        );

        if extracted == 0 {
            return HICON::default();
        }

        if !small_icon.is_invalid() {
            if !large_icon.is_invalid() {
                let _ = DestroyIcon(large_icon);
            }
            small_icon
        } else {
            large_icon
        }
    }
}

/// Show a Windows balloon notification from the single application tray icon.
pub fn notify_balloon(hwnd: HWND, _kind: TrayIconKind, title: &str, message: &str) {
    notify_balloon_with_severity(hwnd, title, message, NoticeSeverity::Warning);
}

pub fn notify_balloon_with_severity(
    hwnd: HWND,
    title: &str,
    message: &str,
    severity: NoticeSeverity,
) {
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = APP_TRAY_ICON_ID;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = match severity {
            NoticeSeverity::Information => NIIF_INFO,
            NoticeSeverity::Warning => NIIF_WARNING,
        };
        copy_wide(title, &mut nid.szInfoTitle);
        copy_wide(message, &mut nid.szInfo);
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

fn copy_wide<const N: usize>(s: &str, buf: &mut [u16; N]) {
    let wide: Vec<u16> = s.encode_utf16().collect();
    let mut len = wide.len().min(N - 1);
    if len > 0 && (0xD800..=0xDBFF).contains(&wide[len - 1]) {
        len -= 1;
    }
    buf[..len].copy_from_slice(&wide[..len]);
    buf[len] = 0;
}

/// Windows allows 127 UTF-16 units plus the terminator. Mark omitted content
/// explicitly without splitting a surrogate pair; the widget tooltip has the
/// full account list.
fn copy_tooltip(s: &str, buf: &mut [u16; 128]) {
    copy_wide(s, buf);
    if s.encode_utf16().count() >= buf.len() {
        let mut end = buf.len() - 2;
        if (0xD800..=0xDBFF).contains(&buf[end - 1]) {
            end -= 1;
        }
        buf[end] = '…' as u16;
        buf[end + 1] = 0;
    }
}

fn add(hwnd: HWND, tooltip: &str) {
    let hicon = create_icon();
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = APP_TRAY_ICON_ID;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_APP_TRAY;
        nid.hIcon = hicon;
        copy_tooltip(tooltip, &mut nid.szTip);
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        if !hicon.is_invalid() {
            let _ = DestroyIcon(hicon);
        }
    }
}

fn update(hwnd: HWND, tooltip: &str) {
    let hicon = create_icon();
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = APP_TRAY_ICON_ID;
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = hicon;
        copy_tooltip(tooltip, &mut nid.szTip);
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        if !hicon.is_invalid() {
            let _ = DestroyIcon(hicon);
        }
    }
}

fn remove_id(hwnd: HWND, id: u32) {
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = id;
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn sync(hwnd: HWND, icon: Option<&TrayIconData>) {
    // Remove provider-specific icons left by versions before v1.9.
    remove_id(hwnd, LEGACY_CODEX_TRAY_ICON_ID);
    remove_id(hwnd, LEGACY_ANTIGRAVITY_TRAY_ICON_ID);

    if let Some(icon) = icon {
        add(hwnd, &icon.tooltip);
        update(hwnd, &icon.tooltip);
    } else {
        remove_id(hwnd, APP_TRAY_ICON_ID);
    }
}

pub fn remove_all(hwnd: HWND) {
    remove_id(hwnd, APP_TRAY_ICON_ID);
    remove_id(hwnd, LEGACY_CODEX_TRAY_ICON_ID);
    remove_id(hwnd, LEGACY_ANTIGRAVITY_TRAY_ICON_ID);
}

/// Interpret a tray callback message and return the action to take.
pub fn handle_message(lparam: LPARAM) -> TrayAction {
    let mouse_msg = lparam.0 as u32;
    match mouse_msg {
        WM_LBUTTONUP => TrayAction::ToggleWidget,
        WM_RBUTTONUP => TrayAction::ShowContextMenu,
        _ => TrayAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::copy_tooltip;

    fn tooltip(text: &str) -> String {
        let mut buffer = [0u16; 128];
        copy_tooltip(text, &mut buffer);
        let end = buffer.iter().position(|unit| *unit == 0).unwrap();
        String::from_utf16(&buffer[..end]).unwrap()
    }

    #[test]
    fn tray_tooltip_preserves_text_that_fits() {
        for text in [String::new(), "Codex: 5h 62%".into(), "x".repeat(127)] {
            assert_eq!(tooltip(&text), text);
        }
    }

    #[test]
    fn tray_tooltip_marks_truncated_accounts() {
        let text = format!("work@example.com: 5h 62%\n{}", "other account\n".repeat(20));
        let result = tooltip(&text);
        assert!(result.starts_with("work@example.com: 5h 62%\n"));
        assert!(result.ends_with('…'));
        assert_eq!(result.encode_utf16().count(), 127);
    }

    #[test]
    fn tray_tooltip_keeps_surrogate_pairs_intact() {
        for prefix in [124, 125, 126] {
            let result = tooltip(&format!("{}😀{}", "x".repeat(prefix), "y".repeat(10)));
            assert!(result.ends_with('…'));
            assert!(result.encode_utf16().count() <= 127);
        }
    }
}
