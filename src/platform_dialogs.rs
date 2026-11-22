//! Common file dialogs (IFileOpenDialog / IFileSaveDialog).

use std::path::PathBuf;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
};

pub fn open_file(name: &str, pattern: &str) -> Option<PathBuf> {
    unsafe {
        let d: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let (n, p) = (HSTRING::from(name), HSTRING::from(pattern));
        let spec = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(n.as_ptr()),
            pszSpec: PCWSTR(p.as_ptr()),
        }];
        let _ = d.SetFileTypes(&spec);
        d.Show(None).ok()?;
        let item = d.GetResult().ok()?;
        let s = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let out = s.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(s.0 as _));
        out
    }
}

pub fn save_file(default_name: &str) -> Option<PathBuf> {
    unsafe {
        let d: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let _ = d.SetFileName(&HSTRING::from(default_name));
        d.Show(None).ok()?;
        let item = d.GetResult().ok()?;
        let s = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let out = s.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(s.0 as _));
        out
    }
}
