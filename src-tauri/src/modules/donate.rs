use anyhow::Result;
use base64::Engine;

// Embed the donation QR (WeChat Pay) into the binary so it cannot be swapped by
// replacing frontend assets on disk. This is "tamper-resistant", not "tamper-proof"
// (a determined user can still patch binaries).
//
// The QR source lives under `src-tauri/assets/` so it is not shipped as a plain
// frontend file (unlike Vite `public/` assets).
//
// NOTE: We embed a JPEG to match the user's original QR image file.
const DONATE_WECHAT_JPG: &[u8] = include_bytes!("../../assets/donate-wechat.jpg");

pub fn wechat_qr_data_url() -> Result<String> {
    // Data URL avoids needing any extra file I/O at runtime.
    let encoded = base64::engine::general_purpose::STANDARD.encode(DONATE_WECHAT_JPG);
    Ok(format!("data:image/jpeg;base64,{encoded}"))
}
