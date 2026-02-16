# Security Notes

## 1. API key handling
- Installer now initializes config through `openclaw onboard --non-interactive`.
- OpenClaw stores provider keys/tokens in:
  - `%USERPROFILE%\.openclaw\.env` (primary)
- Legacy installs may still have key-like fields in:
  - `%USERPROFILE%\.openclaw\openclaw.json`
- UI warns that secrets are plaintext and should use least-privilege keys.

## 2. ACL hardening
- `configure()` attempts to harden ACL for:
  - `%USERPROFILE%\.openclaw\openclaw.json`
  - `%USERPROFILE%\.openclaw\.env` (if present)
- It runs:
  - `icacls /inheritance:r`
  - `icacls /grant:r <current-user>:(R,W)`
- Any ACL failure is surfaced in the wizard result.

## 3. Security scan (`security_check`)
Checks:
- plaintext key patterns in `openclaw.json`
- plaintext key/token patterns in `.env`
- broad ACL (`Everyone`, `BUILTIN\Users` read)
- suspicious script patterns under OpenClaw/runtime folders:
  - `Invoke-Expression`
  - `DownloadString`
  - `FromBase64String`
  - `powershell -enc`

## 4. Operational recommendations
- Rotate API keys periodically.
- Use dedicated low-scope keys for OpenClaw.
- Run backup before upgrade/rollback/model-switch operations.
- Review installer logs after every upgrade or rollback.

## 5. MVP limits
- No built-in encrypted vault in this MVP.
- No centralized SIEM integration.
