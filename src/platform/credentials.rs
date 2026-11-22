//! Secrets: DPAPI-encrypted API keys in secrets.bin, OAuth refresh tokens in
//! Windows Credential Manager, plus BCrypt random and SHA-256 helpers.

use std::collections::BTreeMap;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Credentials::*;
use windows::Win32::Security::Cryptography::*;

use crate::util::{data_dir, wide};

pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
        .ok()?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
        Some(v)
    }
}

pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
        .ok()?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
        Some(v)
    }
}

fn load_secrets() -> BTreeMap<String, String> {
    std::fs::read(data_dir().join("secrets.bin"))
        .ok()
        .and_then(|b| unprotect(&b))
        .and_then(|p| serde_json::from_slice(&p).ok())
        .unwrap_or_default()
}

pub fn get_secret(name: &str) -> Option<String> {
    load_secrets().get(name).cloned()
}

pub fn set_secret(name: &str, value: &str) {
    let mut m = load_secrets();
    if value.is_empty() {
        m.remove(name);
    } else {
        m.insert(name.to_string(), value.to_string());
    }
    if let Some(enc) = serde_json::to_vec(&m).ok().and_then(|j| protect(&j)) {
        let _ = crate::util::atomic_write(&data_dir().join("secrets.bin"), &enc);
    }
}

pub fn has_secret(name: &str) -> bool {
    get_secret(name).is_some_and(|s| !s.is_empty())
}

pub fn cred_write(target: &str, secret: &str) -> Result<(), String> {
    let t = wide(target);
    let user = wide("Notchly");
    let blob = secret.as_bytes();
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(t.as_ptr() as *mut u16),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user.as_ptr() as *mut u16),
        ..Default::default()
    };
    unsafe { CredWriteW(&cred, 0) }.map_err(|e| e.to_string())
}

pub fn cred_read(target: &str) -> Option<String> {
    let t = wide(target);
    unsafe {
        let mut p: *mut CREDENTIALW = std::ptr::null_mut();
        CredReadW(PCWSTR(t.as_ptr()), CRED_TYPE_GENERIC, None, &mut p).ok()?;
        let c = &*p;
        let s = String::from_utf8_lossy(std::slice::from_raw_parts(
            c.CredentialBlob,
            c.CredentialBlobSize as usize,
        ))
        .to_string();
        CredFree(p as _);
        Some(s)
    }
}

pub fn cred_delete(target: &str) {
    let t = wide(target);
    unsafe {
        let _ = CredDeleteW(PCWSTR(t.as_ptr()), CRED_TYPE_GENERIC, None);
    }
}

pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    unsafe {
        let _ = BCryptGenRandom(None, &mut v, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
    }
    v
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    unsafe {
        let _ = BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out);
    }
    out
}

pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn dpapi_roundtrip() {
        let e = protect(b"secret").unwrap();
        assert_eq!(unprotect(&e).unwrap(), b"secret");
    }
    #[test]
    fn random_differs() {
        assert_ne!(random_bytes(16), random_bytes(16));
    }
}
