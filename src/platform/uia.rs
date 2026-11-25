//! UI Automation provider: the island is a Pane named "Notchly" (or the open
//! page's title) whose children are its focusable controls with names, bounds
//! and the Invoke pattern. Pop-ups are announced with UiaRaiseNotificationEvent.
#![allow(non_upper_case_globals)]

use std::cell::RefCell;
use std::mem::ManuallyDrop;

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

#[derive(Default)]
struct Tree {
    hwnd: isize,
    title: String,
    nodes: Vec<Node>,
    root: Option<IRawElementProviderSimple>,
    /// Indices of nodes invoked by assistive tech, drained by the app.
    invoked: Vec<usize>,
}

thread_local! {
    static TREE: RefCell<Tree> = RefCell::new(Tree::default());
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
    let ok = TREE.with(|t| idx < t.borrow().nodes.len());
    ok.then(|| Child { idx }.into())
}

fn root_fragment() -> windows::core::Result<IRawElementProviderFragmentRoot> {
    TREE.with(|t| t.borrow().root.as_ref().and_then(|r| r.cast().ok()))
        .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
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
            UIA_NamePropertyId => variant_bstr(&TREE.with(|t| t.borrow().title.clone())),
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
        let n = TREE.with(|t| t.borrow().nodes.len());
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
        let idx = TREE.with(|t| {
            t.borrow().nodes.iter().rposition(|n| {
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
        let name = TREE.with(|t| {
            t.borrow()
                .nodes
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
            NavigateDirection_Parent => {
                TREE.with(|t| t.borrow().root.as_ref().and_then(|r| r.cast().ok()))
            }
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
        let r = TREE.with(|t| {
            t.borrow()
                .nodes
                .get(self.idx)
                .map(|n| n.rect)
                .unwrap_or_default()
        });
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
        TREE.with(|t| t.borrow_mut().invoked.push(self.idx));
        Ok(())
    }
}

fn ensure_root(hwnd: HWND) -> IRawElementProviderSimple {
    TREE.with(|t| {
        let mut t = t.borrow_mut();
        if t.hwnd != hwnd.0 as isize {
            t.hwnd = hwnd.0 as isize;
            t.root = None;
        }
        let h = t.hwnd;
        t.root
            .get_or_insert_with(|| Root { hwnd: h }.into())
            .clone()
    })
}

/// Replaces the accessible tree (cheap: only stores the snapshot).
pub fn set_tree(hwnd: HWND, title: &str, nodes: Vec<Node>) {
    TREE.with(|t| {
        let mut t = t.borrow_mut();
        t.hwnd = hwnd.0 as isize;
        t.title = title.to_string();
        t.nodes = nodes;
    });
}

/// Indices invoked by assistive technology since the last call.
pub fn take_invoked() -> Vec<usize> {
    TREE.with(|t| std::mem::take(&mut t.borrow_mut().invoked))
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
    let ours = TREE.with(|t| t.borrow().hwnd == hwnd.0 as isize);
    if lp.0 as i32 != UiaRootObjectId || !ours {
        return None;
    }
    let root = ensure_root(hwnd);
    Some(unsafe { UiaReturnRawElementProvider(hwnd, wp, lp, &root) })
}
