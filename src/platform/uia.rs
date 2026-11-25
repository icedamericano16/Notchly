//! UI Automation provider: the island is a Pane named "Notchly" (or the open
//! page's title) whose children are its focusable controls with names, bounds
//! and the Invoke pattern. Pop-ups are announced with UiaRaiseNotificationEvent.
#![allow(non_upper_case_globals)]

use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::sync::Mutex;

use windows::core::{implement, Interface, BSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Variant::*;
use windows::Win32::UI::Accessibility::*;

/// Accessible snapshot of an interactive element (screen pixels).
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub name: String,
    pub rect: (i32, i32, i32, i32),
}

/// The accessible snapshot. Process-wide, not thread-local: UI Automation calls provider
/// methods on its own worker threads, not on the window's thread, so a thread-local tree
/// looked empty to Narrator (no name, no children).
#[derive(Default)]
struct Tree {
    hwnd: isize,
    title: String,
    nodes: Vec<Node>,
    /// Indices of nodes invoked by assistive tech, drained by the app.
    invoked: Vec<usize>,
}

static TREE: Mutex<Tree> = Mutex::new(Tree {
    hwnd: 0,
    title: String::new(),
    nodes: Vec::new(),
    invoked: Vec::new(),
});

fn with_tree<R>(f: impl FnOnce(&mut Tree) -> R) -> R {
    f(&mut TREE.lock().unwrap_or_else(|e| e.into_inner()))
}

thread_local! {
    /// The root provider, kept on the window's thread for WM_GETOBJECT and announcements
    /// (COM interfaces can't be shared between threads; other threads make a fresh one).
    static ROOT: RefCell<Option<(isize, IRawElementProviderSimple)>> = const { RefCell::new(None) };
}

/// A root provider for the current window (fresh objects are equivalent: UI Automation
/// identifies the root by its host window).
fn new_root() -> IRawElementProviderSimple {
    Root {
        hwnd: with_tree(|t| t.hwnd),
    }
    .into()
}

fn variant_bstr(s: &str) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BSTR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    bstrVal: ManuallyDrop::new(BSTR::from(s)),
                },
            }),
        },
    }
}

fn variant_i32(v: i32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { lVal: v },
            }),
        },
    }
}

fn variant_bool(v: bool) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    boolVal: windows::Win32::Foundation::VARIANT_BOOL(if v { -1 } else { 0 }),
                },
            }),
        },
    }
}

fn runtime_id(idx: i32) -> *mut SAFEARRAY {
    unsafe {
        let sa = SafeArrayCreateVector(VT_I4, 0, 2);
        if sa.is_null() {
            return sa;
        }
        let ids = [UiaAppendRuntimeId as i32, idx + 1];
        for (i, v) in ids.iter().enumerate() {
            let ix = i as i32;
            let _ = SafeArrayPutElement(sa, &ix, v as *const i32 as _);
        }
        sa
    }
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct Root {
    hwnd: isize,
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IInvokeProvider
)]
struct Child {
    idx: usize,
}

fn child(idx: usize) -> Option<IRawElementProviderFragment> {
    let ok = with_tree(|t| idx < t.nodes.len());
    ok.then(|| Child { idx }.into())
}

fn root_fragment() -> windows::core::Result<IRawElementProviderFragmentRoot> {
    new_root().cast()
}

impl IRawElementProviderSimple_Impl for Root_Impl {
    fn ProviderOptions(&self) -> windows::core::Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }
    fn GetPatternProvider(
        &self,
        _p: UIA_PATTERN_ID,
    ) -> windows::core::Result<windows::core::IUnknown> {
        Err(windows::core::Error::empty())
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> windows::core::Result<VARIANT> {
        Ok(match id {
            UIA_NamePropertyId => variant_bstr(&with_tree(|t| t.title.clone())),
            UIA_ControlTypePropertyId => variant_i32(UIA_PaneControlTypeId.0),
            UIA_AutomationIdPropertyId => variant_bstr("NotchlyIsland"),
            _ => VARIANT::default(),
        })
    }
    fn HostRawElementProvider(&self) -> windows::core::Result<IRawElementProviderSimple> {
        unsafe { UiaHostProviderFromHwnd(HWND(self.hwnd as _)) }
    }
}

impl IRawElementProviderFragment_Impl for Root_Impl {
    fn Navigate(
        &self,
        dir: NavigateDirection,
    ) -> windows::core::Result<IRawElementProviderFragment> {
        let n = with_tree(|t| t.nodes.len());
        let r = match dir {
            NavigateDirection_FirstChild if n > 0 => child(0),
            NavigateDirection_LastChild if n > 0 => child(n - 1),
            _ => None,
        };
        r.ok_or_else(windows::core::Error::empty)
    }
    fn GetRuntimeId(&self) -> windows::core::Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> windows::core::Result<UiaRect> {
        Ok(UiaRect::default())
    }
    fn GetEmbeddedFragmentRoots(&self) -> windows::core::Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> windows::core::Result<IRawElementProviderFragmentRoot> {
        root_fragment()
    }
}

impl IRawElementProviderFragmentRoot_Impl for Root_Impl {
    fn ElementProviderFromPoint(
        &self,
        x: f64,
        y: f64,
    ) -> windows::core::Result<IRawElementProviderFragment> {
        let idx = with_tree(|t| {
            t.nodes.iter().rposition(|n| {
                (x as i32) >= n.rect.0
                    && (x as i32) < n.rect.0 + n.rect.2
                    && (y as i32) >= n.rect.1
                    && (y as i32) < n.rect.1 + n.rect.3
            })
        });
        idx.and_then(child).ok_or_else(windows::core::Error::empty)
    }
    fn GetFocus(&self) -> windows::core::Result<IRawElementProviderFragment> {
        Err(windows::core::Error::empty())
    }
}

impl IRawElementProviderSimple_Impl for Child_Impl {
    fn ProviderOptions(&self) -> windows::core::Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }
    fn GetPatternProvider(
        &self,
        p: UIA_PATTERN_ID,
    ) -> windows::core::Result<windows::core::IUnknown> {
        if p == UIA_InvokePatternId {
            let me: IInvokeProvider = Child { idx: self.idx }.into();
            return me.cast();
        }
        Err(windows::core::Error::empty())
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> windows::core::Result<VARIANT> {
        let name = with_tree(|t| {
            t.nodes
                .get(self.idx)
                .map(|n| n.name.clone())
                .unwrap_or_default()
        });
        Ok(match id {
            UIA_NamePropertyId => variant_bstr(&name),
            UIA_ControlTypePropertyId => variant_i32(UIA_ButtonControlTypeId.0),
            UIA_IsKeyboardFocusablePropertyId | UIA_IsControlElementPropertyId => {
                variant_bool(true)
            }
            _ => VARIANT::default(),
        })
    }
    fn HostRawElementProvider(&self) -> windows::core::Result<IRawElementProviderSimple> {
        Err(windows::core::Error::empty())
    }
}

impl IRawElementProviderFragment_Impl for Child_Impl {
    fn Navigate(
        &self,
        dir: NavigateDirection,
    ) -> windows::core::Result<IRawElementProviderFragment> {
        let r = match dir {
            NavigateDirection_Parent => new_root().cast().ok(),
            NavigateDirection_NextSibling => child(self.idx + 1),
            NavigateDirection_PreviousSibling if self.idx > 0 => child(self.idx - 1),
            _ => None,
        };
        r.ok_or_else(windows::core::Error::empty)
    }
    fn GetRuntimeId(&self) -> windows::core::Result<*mut SAFEARRAY> {
        Ok(runtime_id(self.idx as i32))
    }
    fn BoundingRectangle(&self) -> windows::core::Result<UiaRect> {
        let r = with_tree(|t| t.nodes.get(self.idx).map(|n| n.rect).unwrap_or_default());
        Ok(UiaRect {
            left: r.0 as f64,
            top: r.1 as f64,
            width: r.2 as f64,
            height: r.3 as f64,
        })
    }
    fn GetEmbeddedFragmentRoots(&self) -> windows::core::Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> windows::core::Result<IRawElementProviderFragmentRoot> {
        root_fragment()
    }
}

impl IInvokeProvider_Impl for Child_Impl {
    fn Invoke(&self) -> windows::core::Result<()> {
        with_tree(|t| t.invoked.push(self.idx));
        Ok(())
    }
}

fn ensure_root(hwnd: HWND) -> IRawElementProviderSimple {
    let h = hwnd.0 as isize;
    with_tree(|t| t.hwnd = h);
    ROOT.with(|r| {
        let mut r = r.borrow_mut();
        match r.as_ref() {
            Some((rh, root)) if *rh == h => root.clone(),
            _ => {
                let root: IRawElementProviderSimple = Root { hwnd: h }.into();
                *r = Some((h, root.clone()));
                root
            }
        }
    })
}

/// Replaces the accessible tree (cheap: only stores the snapshot).
pub fn set_tree(hwnd: HWND, title: &str, nodes: Vec<Node>) {
    with_tree(|t| {
        t.hwnd = hwnd.0 as isize;
        t.title = title.to_string();
        t.nodes = nodes;
    });
}

/// Indices invoked by assistive technology since the last call.
pub fn take_invoked() -> Vec<usize> {
    with_tree(|t| std::mem::take(&mut t.invoked))
}

/// Announces text to screen readers (only when a client is listening).
pub fn announce(hwnd: HWND, text: &str) {
    if text.trim().is_empty() || !unsafe { UiaClientsAreListening() }.as_bool() {
        return;
    }
    let root = ensure_root(hwnd);
    unsafe {
        let _ = UiaRaiseNotificationEvent(
            &root,
            NotificationKind_ActionCompleted,
            NotificationProcessing_ImportantMostRecent,
            &BSTR::from(text),
            &BSTR::from("notchly"),
        );
    }
}

pub fn on_get_object(hwnd: HWND, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    let ours = with_tree(|t| t.hwnd == hwnd.0 as isize);
    if lp.0 as i32 != UiaRootObjectId || !ours {
        return None;
    }
    let root = ensure_root(hwnd);
    Some(unsafe { UiaReturnRawElementProvider(hwnd, wp, lp, &root) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old bug: the tree lived in a thread-local, and UI Automation reads providers on
    /// its own threads, so Narrator saw no name and no children.
    #[test]
    fn the_tree_is_visible_from_other_threads() {
        set_tree(
            HWND(std::ptr::null_mut()),
            "Weather",
            vec![Node {
                name: "Refresh".into(),
                rect: (1, 2, 3, 4),
            }],
        );
        let seen = std::thread::spawn(|| {
            with_tree(|t| (t.title.clone(), t.nodes.first().map(|n| n.name.clone())))
        })
        .join()
        .unwrap();
        assert_eq!(seen, ("Weather".to_string(), Some("Refresh".to_string())));
        // invocations from a UIA thread reach the app's thread
        std::thread::spawn(|| with_tree(|t| t.invoked.push(0)))
            .join()
            .unwrap();
        assert_eq!(take_invoked(), vec![0]);
    }
}
