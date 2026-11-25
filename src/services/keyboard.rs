//! Low-level keyboard hook on its own thread. It only looks at Caps/Num Lock and
//! the input-language switch combos; it never records keystrokes.

use std::sync::OnceLock;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CAPITAL, VK_CONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NUMLOCK,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::{spawn, Bus, Event};
use crate::util::log;

static BUS: OnceLock<Bus> = OnceLock::new();

fn down(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

unsafe extern "system" fn hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wp.0 as u32 == WM_KEYUP || wp.0 as u32 == WM_SYSKEYUP) {
        // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lParam points to a KBDLLHOOKSTRUCT valid
        // for this call.
        let k = unsafe { &*(lp.0 as *const KBDLLHOOKSTRUCT) };
        let vk = k.vkCode as u16;
        if let Some(bus) = BUS.get() {
            if vk == VK_CAPITAL.0 || vk == VK_NUMLOCK.0 {
                bus.send(Event::LockKey(vk as u32));
            } else {
                let win_space = vk == VK_SPACE.0 && (down(VK_LWIN.0) || down(VK_RWIN.0));
                let shift_combo = (vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 || vk == VK_SHIFT.0)
                    && (down(VK_MENU.0) || down(VK_CONTROL.0));
                let alt_combo =
                    (vk == VK_LMENU.0 || vk == VK_RMENU.0 || vk == VK_MENU.0 || vk == VK_CONTROL.0)
                        && (down(VK_LSHIFT.0) || down(VK_RSHIFT.0));
                if win_space || shift_combo || alt_combo {
                    bus.send(Event::LangHotkey);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wp, lp) }
}

pub fn start(bus: Bus) {
    if BUS.set(bus).is_err() {
        return;
    }
    spawn("kbd-hook", || unsafe {
        let h = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) {
            Ok(h) => h,
            Err(e) => {
                log(&format!("keyboard hook failed: {e}"));
                return;
            }
        };
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWindowsHookEx(h);
    });
}
