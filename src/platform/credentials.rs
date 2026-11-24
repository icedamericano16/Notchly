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

/// Verifies an ECDSA P-256 / SHA-256 signature (IEEE P1363 r||s, 64 bytes) of `msg` with a
/// public key given as X||Y (64 bytes), using Windows CNG. False on any error.
pub fn verify_p256(public_xy: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    if public_xy.len() != 64 || sig.len() != 64 {
        return false;
    }
    let mut blob = Vec::with_capacity(72);
    blob.extend_from_slice(&BCRYPT_ECDSA_PUBLIC_P256_MAGIC.to_le_bytes());
    blob.extend_from_slice(&32u32.to_le_bytes());
    blob.extend_from_slice(public_xy);
    let hash = sha256(msg);
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        if BCryptOpenAlgorithmProvider(
            &mut alg,
            BCRYPT_ECDSA_P256_ALGORITHM,
            PCWSTR::null(),
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
        .is_err()
        {
            return false;
        }
        let mut key = BCRYPT_KEY_HANDLE::default();
        let ok = BCryptImportKeyPair(alg, None, BCRYPT_ECCPUBLIC_BLOB, &mut key, &blob, 0).is_ok()
            && BCryptVerifySignature(key, None, &hash, sig, BCRYPT_FLAGS(0)).is_ok();
        if !key.is_invalid() {
            let _ = BCryptDestroyKey(key);
        }
        let _ = BCryptCloseAlgorithmProvider(alg, 0);
        ok
    }
}

/// Decodes base64 (standard or URL-safe alphabet, padding optional). None if malformed.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.trim().bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6 | v) & 0xFF_FFFF;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Signed with packaging/sign-update.ps1 (.NET ECDsa) and a throw-away key: the format
    /// the release pipeline produces must verify here, and nothing else may.
    #[test]
    fn verifies_manifests_signed_by_the_release_script() {
        let public = b64_decode(
            "AjSTq4VnxPQnRcNvQvc8Dp2oYGS/wZNqTzE9S4VkikhWOdWO3ePH50zu32kfwBkP/+OKHBh+GjZZeJ+It6SlEw==",
        )
        .unwrap();
        let sig = b64_decode(
            "S5lXRMp61OJDFo2L23GZ7BkpXpVgG4zPuUIlSWpkBahzVca3gP9MuJWl/9B0JC2WSmY+cpose2755R5nayOg0g==",
        )
        .unwrap();
        let msg = br#"{"version":"9.9.9","url":"https://example.com/x.exe","sha256":"00"}"#;
        assert!(verify_p256(&public, msg, &sig));
        // a changed byte, a changed signature or another key all fail
        let mut tampered = msg.to_vec();
        tampered[13] = b'8';
        assert!(!verify_p256(&public, &tampered, &sig));
        let mut bad = sig.clone();
        bad[5] ^= 1;
        assert!(!verify_p256(&public, msg, &bad));
        let mut other = public.clone();
        other[63] ^= 1;
        assert!(!verify_p256(&other, msg, &sig));
        assert!(!verify_p256(&public[..32], msg, &sig));
    }

    #[test]
    fn base64() {
        assert_eq!(b64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(b64_decode("aGVsbG8").unwrap(), b"hello");
        assert_eq!(b64_decode("-_-_").unwrap(), b64_decode("+/+/").unwrap());
        assert!(b64_decode("a*b").is_none());
    }

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
