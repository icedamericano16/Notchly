//! Drag & drop: OLE drop target on the island (files, text, links) and drag-out
//! of shelf items through a shell data object.

use std::cell::RefCell;

use windows::core::{implement, Interface, Ref, HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, HWND, POINTL, S_OK,
};
use windows::Win32::System::Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::*;

use crate::util::wide;

#[derive(Debug, Clone, PartialEq)]
pub enum DropMsg {
    Enter,
    Leave,
    Files(Vec<String>),
    Text(String),
}

thread_local! {
    pub static DROPS: RefCell<Vec<DropMsg>> = const { RefCell::new(Vec::new()) };
}

fn push(m: DropMsg) {
    DROPS.with(|d| d.borrow_mut().push(m));
}

pub fn hdrop_files(h: HDROP) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let n = DragQueryFileW(h, u32::MAX, None);
        for i in 0..n {
            let len = DragQueryFileW(h, i, None);
            let mut buf = vec![0u16; len as usize + 1];
            DragQueryFileW(h, i, Some(&mut buf));
            out.push(crate::util::from_wide(&buf));
        }
    }
    out
}

fn has_format(obj: &IDataObject, cf: u16) -> bool {
    let f = FORMATETC {
        cfFormat: cf,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    unsafe { obj.QueryGetData(&f) == S_OK }
}

fn extract(obj: &IDataObject) -> Option<DropMsg> {
    unsafe {
        let f = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        if let Ok(mut m) = obj.GetData(&f) {
            let files = hdrop_files(HDROP(m.u.hGlobal.0));
            ReleaseStgMedium(&mut m);
            if !files.is_empty() {
                return Some(DropMsg::Files(files));
            }
        }
        let f = FORMATETC {
            cfFormat: CF_UNICODETEXT.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        if let Ok(mut m) = obj.GetData(&f) {
            let g = m.u.hGlobal;
            let p = GlobalLock(g) as *const u16;
            let mut text = String::new();
            if !p.is_null() {
                // SAFETY: GlobalLock succeeded, so `p` addresses GlobalSize bytes, locked until
                // GlobalUnlock below. The scan stops at the block's end even without a NUL.
                let max = windows::Win32::System::Memory::GlobalSize(g) / 2;
                let mut n = 0;
                while n < max && *p.add(n) != 0 {
                    n += 1;
                }
                text = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                let _ = GlobalUnlock(g);
            }
            ReleaseStgMedium(&mut m);
            if !text.trim().is_empty() {
                return Some(DropMsg::Text(text));
            }
        }
    }
    None
}

#[implement(IDropTarget)]
struct Target;

impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        obj: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let ok = obj
            .as_ref()
            .is_some_and(|o| has_format(o, CF_HDROP.0) || has_format(o, CF_UNICODETEXT.0));
        unsafe { *effect = if ok { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        if ok {
            push(DropMsg::Enter);
        }
        Ok(())
    }
    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe { *effect = DROPEFFECT_COPY };
        Ok(())
    }
    fn DragLeave(&self) -> windows::core::Result<()> {
        push(DropMsg::Leave);
        Ok(())
    }
    fn Drop(
        &self,
        obj: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe { *effect = DROPEFFECT_COPY };
        push(DropMsg::Leave);
        if let Some(m) = obj.as_ref().and_then(extract) {
            push(m);
        }
        Ok(())
    }
}

pub fn register(hwnd: HWND) {
    let t: IDropTarget = Target.into();
    unsafe {
        let _ = RegisterDragDrop(hwnd, &t);
    }
}

#[implement(IDropSource)]
struct Source;

impl IDropSource_Impl for Source_Impl {
    fn QueryContinueDrag(
        &self,
        escape: windows::core::BOOL,
        keys: MODIFIERKEYS_FLAGS,
    ) -> windows::core::HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }
    fn GiveFeedback(&self, _effect: DROPEFFECT) -> windows::core::HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// Builds a shell data object for files (CF_HDROP + shell IDList formats).
pub fn data_object(paths: &[String]) -> Option<IDataObject> {
    unsafe {
        let mut pidls = Vec::new();
        for p in paths {
            let w = wide(p);
            let mut pidl: *mut windows::Win32::UI::Shell::Common::ITEMIDLIST = std::ptr::null_mut();
            if SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut pidl, 0, None).is_ok() {
                pidls.push(pidl as *const _);
            }
        }
        if pidls.is_empty() {
            return None;
        }
        let arr = SHCreateShellItemArrayFromIDLists(&pidls).ok();
        for p in &pidls {
            windows::Win32::System::Com::CoTaskMemFree(Some(*p as _));
        }
        arr?.BindToHandler(None, &BHID_DataObject).ok()
    }
}

/// Starts a modal drag of files into any app. Returns when the drag ends.
pub fn drag_out(paths: &[String]) -> bool {
    let Some(obj) = data_object(paths) else {
        return false;
    };
    let src: IDropSource = Source.into();
    let mut effect = DROPEFFECT_NONE;
    unsafe {
        DoDragDrop(&obj, &src, DROPEFFECT_COPY | DROPEFFECT_LINK, &mut effect) == DRAGDROP_S_DROP
    }
}

/// Windows Share UI for files.
pub fn share(hwnd: HWND, paths: Vec<String>) {
    use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    unsafe {
        let Ok(interop) =
            windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()
        else {
            return;
        };
        let Ok(dtm): windows::core::Result<DataTransferManager> = interop.GetForWindow(hwnd) else {
            return;
        };
        let _ = dtm.DataRequested(&TypedEventHandler::new(
            move |_, args: Ref<DataRequestedEventArgs>| {
                if let Some(a) = args.as_ref() {
                    let req = a.Request()?;
                    let data = req.Data()?;
                    data.Properties()?
                        .SetTitle(&HSTRING::from("Notchly shelf"))?;
                    let mut items: Vec<Option<IStorageItem>> = Vec::new();
                    for p in &paths {
                        if let Ok(f) = StorageFile::GetFileFromPathAsync(&HSTRING::from(p.as_str()))
                            .and_then(|o| o.join())
                        {
                            items.push(f.cast().ok());
                        }
                    }
                    let list = windows_collections::IIterable::<IStorageItem>::from(items);
                    data.SetStorageItemsReadOnly(&list)?;
                }
                Ok(())
            },
        ));
        let _ = interop.ShowShareUIForWindow(hwnd);
    }
}
