//! QA helper: holds a microphone capture stream open for N seconds (nothing is read or kept),
//! so the In use detection can be checked. Run: mic_probe <seconds>

use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let en: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();
        let dev = en.GetDefaultAudioEndpoint(eCapture, eConsole).unwrap();
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None).unwrap();
        let fmt = client.GetMixFormat().unwrap();
        client
            .Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 10_000_000, 0, fmt, None)
            .unwrap();
        client.Start().unwrap();
        println!("capturing (discarded) for {secs} s");
        std::thread::sleep(std::time::Duration::from_secs(secs));
        let _ = client.Stop();
    }
}
