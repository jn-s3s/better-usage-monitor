# Troubleshooting Codex Usage

## Taskbar error labels

Codex Usage keeps authentication failures separate from transient service failures:

| Simplified Chinese | Other languages | Meaning                                                                             | Recommended action                                                                                                         |
| ------------------ | --------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `!`                | `!`             | Enabled service credentials are missing or expired                                  | Sign in with the relevant CLI/app, then refresh Codex Usage. Claude Desktop login does not count as Claude Code CLI login. |
| `网络`             | `NET`           | Network or TLS connection failed                                                    | Check connectivity, VPN, proxy, and firewall settings                                                                      |
| `限流`             | `429`           | Provider rate limit                                                                 | Wait for the provider retry window; Codex Usage retries with backoff                                                       |
| `服务`             | `5XX`           | Provider service failure                                                            | Wait and retry; check provider status if it persists                                                                       |
| `账号`             | `ACCT`          | The usage endpoint answered for a different account than the configured Codex login | Check `codex_home` in `codex_accounts`; the value must point at the profile that holds that login's `auth.json`            |
| `错误`             | `ERR`           | Invalid or unsupported response                                                     | Enable diagnostics and inspect the log                                                                                     |

Authentication failures pause provider polling until the credential source changes, preventing repeated login notifications. Transient failures use exponential backoff up to the configured refresh interval.

## Extra Codex accounts

Each extra Codex login is a separate Codex CLI profile directory with its own `auth.json`. Use **Codex accounts → Add account…** in the right-click menu, or list the profile in `codex_accounts` in `settings.json` (see [Multiple Codex Accounts](../README.md#multiple-codex-accounts)).

- **Add account… turned into Cancel sign-in**: a sign-in started earlier is still being watched. Choose it to stop waiting for it; otherwise the monitor gives up by itself after five minutes.
- **The Codex window opened and closed before it could be read**: the CLI started and exited, most often because its profile directory did not exist yet (`CODEX_HOME points to "…", but that path does not exist`). **Add account…** creates the folder before starting the CLI, so this only happens for a directory you created by hand; make sure it exists before running `codex login`.
- **Nothing happened after Add account…**: the Codex CLI could not be started, or its console closed immediately. Run `codex login` yourself with `CODEX_HOME` pointing at the profile directory and add the folder to `codex_accounts`.
- **A sign-in finished but no account appeared**: the profile folder is named `~/.codex-account2`, `~/.codex-account3`, and so on. If the credentials landed there after the five-minute window, add that folder to `codex_accounts` manually.
- **A balloon says the Codex account limit is reached**: the app monitors up to 39 Codex logins including the default. Remove an account before adding another, or drop an entry from `codex_accounts`; entries past the limit are ignored.
- `账号` / `ACCT` for one login: the endpoint returned someone else's usage. Verify that `codex_home` is the profile you logged in with. Two profile directories pointing at the same login both report the same account.
- `!` for one login: that profile has no usable `auth.json`. Create the profile directory if needed, run `codex login` with `CODEX_HOME` pointing at it, then choose **Refresh**.
- The login disappears from the tooltip and tray summary: it was removed from `codex_accounts`, or Codex is hidden. Hiding Codex stops every Codex login from polling.
- The widget shows the wrong login: pick another entry from **Codex accounts** in the right-click menu. The selection is saved to `active_codex_account` and survives restarts.
- **Remove account** only unregisters the login. The profile folder and its credentials stay on disk, so the login can be added again later.
- An extra login refreshes through the Codex CLI when its token expires, which spends one model request like any other refresh.

## Diagnostic log

Run:

```powershell
codex-usage.exe --diagnose
```

The log is written to `%TEMP%\codex-usage.log`. It includes:

- application version and executable path
- install channel (direct or portable)
- provider failure category and retry delay
- window creation, taskbar placement, and relaunch events

The log does not include access tokens, refresh tokens, credential file contents, or API response bodies.

## Update failures

Direct installations and portable copies download only the exact `codex-usage.exe` asset and verify it against `codex-usage.exe.sha256` from the same GitHub Release. The updater keeps the previous EXE until the downloaded version has been installed and restarted successfully. If restart fails, the old EXE is restored.

## Reset local position without deleting settings

Right-click the taskbar component and choose **Settings > Reset Position**. Settings are stored at `%APPDATA%\CodexUsage\settings.json`.

## Reinstall while preserving settings

Normal uninstall keeps the settings file. Reinstalling restores the saved language, refresh interval, provider selection, widget visibility, and taskbar position. Use `uninstall.ps1 -RemoveSettings` only when a full reset is intended.
