![Windows](https://img.shields.io/badge/platform-Windows-blue)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**English** | [简体中文](README.zh-CN.md)

# Codex Usage

<img src=".github/codex-usage-icon.png" alt="Codex Usage icon" width="96" height="96">

![Screenshot](.github/animation.gif)

This animation shows the original segmented-bar design. Current builds let you choose segmented or continuous bars in **Appearance**.

A lightweight native Windows taskbar widget for monitoring Codex usage, with optional Claude Code and Google Antigravity usage display.

It sits in your taskbar and shows how much of your Codex usage window remains without opening the Codex app or account usage page.

## What You Get

- A **5h** bar for your current Codex usage window
- A **7d** bar for your current weekly window
- Simplified Chinese display with explicit remaining usage and reset countdowns
- Optional Claude Code usage alongside Codex
- Optional Antigravity model usage bars for Google's 5-hour and weekly Gemini quota windows
- A live countdown until each limit resets
- Optional low-quota alerts at 10%, 20%, or 30% remaining, deduplicated per reset window
- Independent display controls for the 5-hour and weekly rows
- A small native widget that lives directly in the Windows taskbar
- One system tray icon that matches the desktop app icon
- Left-click the tray icon to toggle the taskbar widget on or off
- Right-click options for refresh, monitored services, usage rows, quota alerts, update frequency, language, startup, widget visibility, and updates
- Multi-monitor taskbar placement, so the widget can live on the taskbar for the screen you prefer
- Appearance options for palette, bar style, bar thickness, and text size
- Claude and OpenAI marks for each service column, with Codex shown in green
- Quota windows that a service does not report show `--` and an empty bar instead of a false 100%
- Each service refreshes and retries on its own; a failed refresh keeps the last numbers and marks them `*`
- Hover over a service column to see its last successful update, next refresh, and any error
- Automatic refresh after the computer wakes from sleep or the network comes back

## Who This Is For

This app is for Windows users who already have **Codex CLI or the Codex app installed and signed in**.

Codex is enabled by default. The app reads the same local credentials used by Codex.

Antigravity support is optional too. To show Antigravity usage, install and sign in to Google Antigravity, then enable the **Antigravity** service from the right-click **Monitored services** menu.

It works best if you want a simple "how close am I to the limit?" display that is always visible.

## Requirements

- Windows 10 or Windows 11
- Codex CLI or Codex app installed and authenticated
- Optional: Claude Code installed and authenticated
- Optional: Google Antigravity installed and authenticated, if you want Antigravity usage

Refresh after network recovery needs Windows 10 version 2004 or later. On earlier Windows 10 builds the app still runs, but it only refreshes after wake from sleep and on its normal schedule.

If you use Claude Code through WSL, that is supported too. The monitor can read your Claude Code credentials from Windows or from your WSL environment.

## Install

For a per-user installation, download `install.ps1` from the [latest release](https://github.com/upstream-ray/codex-usage-monitor/releases/latest), then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

The installer verifies the release SHA256 and installs to `%LOCALAPPDATA%\Programs\CodexUsage` without administrator access. It adds a Start menu shortcut and an entry in Windows Installed Apps.

For portable use, download `codex-usage.exe` from the same release and run it from any user-writable directory. You can also build it locally:

```powershell
cargo build --release
```

Local builds create the executable at `target\release\codex-usage.exe`.

### Development checks

`dev` is this fork's default branch. Create feature or fix branches and submit all
development PRs to `dev`; direct pushes to `dev` are blocked, including for admins.
Keep `main` as an upstream mirror: do not target it with development PRs or add
fork-specific commits. Updates to `main` are restricted to repository admins for
deliberate upstream synchronization; that bypass also permits other updates, so
admins must preserve the mirror policy.

Every pull request targeting `dev`, including draft PRs, runs these checks on Windows
with stable Rust. Run the same checks locally before submitting changes:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

## Uninstall

Uninstall **Codex Usage** from Windows Settings > Apps > Installed apps, or run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$env:LOCALAPPDATA\Programs\CodexUsage\uninstall.ps1"
```

Uninstalling preserves `%APPDATA%\CodexUsage\settings.json`. Add `-RemoveSettings` to delete settings explicitly. See [Installation model](docs/installation.md) for upgrade, portable, and startup behavior.

## Use

Run:

```powershell
codex-usage
```

Once running, it will appear in your taskbar and as one tray icon in the notification area.

- Drag the left divider to move the taskbar widget
- Choose **Settings → Pin to the left edge of the taskbar** to keep the widget at the left edge instead of next to the tray
- On multi-monitor setups, choose **Settings → Display** from the right-click menu or drag the widget onto another taskbar. The selection is saved; the primary taskbar is used while the selected display is disconnected, and the widget returns when it reconnects.
- Right-click the taskbar widget or tray icon for refresh, monitored services, usage rows, quota alerts, update frequency, Start with Windows, reset position, language, updates, and exit
- Left-click the tray icon to toggle the taskbar widget on or off
- Enable `Start with Windows` from the right-click menu if you want it to launch automatically when you sign in

### Monitored Services

Use the right-click **Monitored services** menu to choose which independent services the widget displays. The services are not mutually exclusive, so you can monitor more than one account at the same time:

- **Codex** is enabled by default
- **Claude Code** can be enabled alongside Codex or shown by itself when Claude Code CLI is installed and authenticated
- **Antigravity** can be enabled alongside the other providers or shown by itself as its own service column

When multiple services are shown, each service has its own usage bar and matching usage text color. Antigravity prefers Google's Gemini quota summary when available and falls back to model quota data when needed.

Claude Desktop and Claude Code CLI use separate local sessions. Signing in to Claude Desktop does not enable Claude Code monitoring. When no supported Claude Code CLI credentials are available, the menu shows **Claude Code (CLI login required)** as a disabled item and automatically keeps that service off.

### Multiple Codex Accounts

The widget shows one Codex login at a time. Right-click the widget or the tray icon and open **Codex accounts** to manage the rest:

- **Add account…** starts `codex login` for a new profile folder (`~/.codex-account2`, `~/.codex-account3`, …) and then waits for its credentials. The Codex CLI opens its own console and your browser: finish the sign-in there and the login is added and selected automatically. Nothing is added when the sign-in does not finish within five minutes. Up to 39 Codex logins (including the default) can be monitored, so once that many are configured this menu entry reports the limit instead of starting a sign-in; extra entries in `codex_accounts` are ignored.
- While that sign-in is running, the same menu entry reads **Stop waiting for sign-in**. This stops the monitor's wait, not the CLI or browser. Close abandoned Codex windows yourself.
- Pick any listed login to move the widget to it. The choice is saved, so it survives a restart.
- **Remove account: …** appears for the login the widget currently shows, never for your default login. For app-managed numbered profiles directly under your home directory, confirmation permanently deletes the folder and credentials; signing in again is required. Custom folders, links/junctions and folders shared with another configured login are only unregistered, not deleted. If deletion fails, the account stays monitored so you can close any Codex windows using it and retry.
- **Use saved profile** lists authenticated profiles that are not currently monitored, including logins saved by older versions or completed after cancellation or the five-minute timeout. Deleted profiles cannot be restored; use **Add account…** to sign in again. The menu is unavailable while another sign-in is pending or the account limit is reached.

Signing into an account already monitored selects its existing entry instead of adding a duplicate, and the duplicate staged attempt is discarded when possible. Matching uses the stored account ID and, when available, user subject rather than the display email. Duplicate settings entries and paths that resolve to the same directory are also filtered. Each new sign-in reserves a fresh staging folder, so an older CLI window cannot overwrite a retry's credentials; close abandoned CLI windows yourself.

Sign-in attempts start in separate folders under `~/.codex-login-attempts`, not in numbered account folders. Only a completed, nonduplicate login is moved to the next available `~/.codex-account<N>` folder. Unfinished attempts remain in the staging area because their CLI windows can still finish; **Use saved profile** also finds completed staged logins. Existing folders are never overwritten; numbers freed by removal can be used for a new successful sign-in. A duplicate sign-in does not replace the existing profile's credentials. To reconnect an extra account, remove it and add it again. For your default login, run `codex login` in Codex CLI and refresh the app. Adding an account while Codex is hidden selects it without enabling Codex; enable it under **Monitored services** to display it.

What the multi-account view gives you:

- The taskbar bars, the widget column, and the tray summary use the selected login
- Every configured login is polled and listed in the Codex column tooltip; the tray summary uses Windows' length-limited tooltip and may truncate when several accounts or services are shown
- Quota alerts follow the selected login: only the login the widget displays raises a low-quota balloon, so an account you are not looking at stays quiet

The browser decides which account gets authorized, because the app only runs the Codex CLI. Sign in with the account you want to monitor. Your default login is always the first entry and is never removed; once another login is configured it is listed under its own account email with a `(main)` marker, falling back to `Codex (main)` when its credentials hold no email. A single-login install keeps showing only `Codex`.

Extra logins can also be added by hand in `%APPDATA%\CodexUsage\settings.json`, which is useful for a profile you already created:

1. Create the profile directory and sign in to it, pointing `CODEX_HOME` at that directory:

    ```powershell
    New-Item -ItemType Directory -Force "$env:USERPROFILE\.codex-work" | Out-Null
    & {
        $previousHome = $env:CODEX_HOME
        try {
            $env:CODEX_HOME = "$env:USERPROFILE\.codex-work"
            codex login
        } finally {
            if ($null -eq $previousHome) {
                Remove-Item Env:CODEX_HOME -ErrorAction SilentlyContinue
            } else {
                $env:CODEX_HOME = $previousHome
            }
        }
    }
    ```

    The directory has to exist first: `codex login` refuses to start with `CODEX_HOME points to … but that path does not exist`. The script sets `CODEX_HOME` only for this sign-in and restores its previous value even on failure, so subsequently launching the monitor from the same shell does not change its default account.

2. List the profile:

    ```json
    {
        "codex_accounts": [{ "label": "work", "codex_home": "C:\\Users\\you\\.codex-work" }],
        "active_codex_account": 1
    }
    ```

    `codex_home` must be an absolute path. `label` is optional; without it the app shows the email from the profile's stored ID token, and falls back to the folder name.

3. Restart the app.

`active_codex_account` selects the login the widget shows: `0` is your default login, `1` is the first entry of `codex_accounts`, and so on. The default login is always index 0 and is never stored in `codex_accounts`.

What to expect:

- The app reads each profile's `auth.json` and never writes credentials. When a profile token expires, the app asks the Codex CLI to refresh it with `CODEX_HOME` set to that profile, exactly like the default login.
- The usage endpoint answers a request for another account with the token's own numbers, so the app checks the account id in the response. A login whose numbers cannot be attributed to it reports `ACCT` (Chinese: `账号`) instead of showing the wrong account's data.
- Hiding Codex stops every Codex login from polling, not just the displayed one.

### System Tray Icon

The app always shows one tray icon using the same embedded icon as the executable and desktop shortcut, regardless of how many services are enabled.

Hovering over the tray icon shows a compact summary for all enabled services, with one line per configured Codex login. Left-clicking it toggles the taskbar widget; right-clicking it opens the settings menu.

### Usage Display And Alerts

Use the right-click **Usage display** menu to show both quota rows or only one. The app always keeps at least one row visible.

Use **Quota alerts** to choose a remaining-quota threshold of 10%, 20%, or 30%. Alerts are off by default. Each provider and quota window is notified only once until its reset time changes, including across app restarts. Codex alerts follow the login the widget displays: the other configured logins are still polled for their numbers but do not raise alerts of their own.

In Simplified Chinese, the compact taskbar rows use `5h` / `7d`, remaining percentage, and a concrete local reset value such as `18:30重置` or `07/17重置`.

Percentages use fixed digit columns, so the reset text does not shift when a value changes from `9%` to `10%`.

### Refresh And Status

Each enabled service keeps its own data, last successful update, error, and retry schedule. A slow or failing service does not delay or replace the numbers of another service.

| Display                                                                                           | Meaning                                                                                                                                                                                 |
| ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--` and an empty bar                                                                             | The service did not report this quota window. For example, some Codex accounts only have a weekly limit.                                                                                |
| `*` after the reset text                                                                          | The latest refresh failed or the data is old. The numbers are from the last successful refresh.                                                                                         |
| `!`, `NET`, `429`, `5XX`, `ACCT`, or `ERR` (Chinese: `!`, `网络`, `限流`, `服务`, `账号`, `错误`) | The service has not returned data yet since the app started, and the label shows why. `ACCT` means the usage endpoint answered for a different account than the configured Codex login. |

Hover over a service column to see when it last updated successfully, when it refreshes or retries next, and the latest error. Missing Codex windows also get a short explanation.

Refresh behavior:

- **Update frequency** sets the normal interval. **Refresh every minute when remaining ≤20%** is on by default. It refreshes a service every minute while one of its quota windows has 20% or less left. A service with a used-up window keeps the normal interval, because it cannot use more quota until the reset.
- Network and server errors retry with increasing delays. When a server answers `429` with a `Retry-After` time, the app waits until that time.
- When a login expires or credentials are missing, the app stops retrying that service until its credentials file changes or you choose **Refresh**.
- Shortly after a quota window resets, the app checks again so the new window shows quickly.

### Appearance

Right-click the widget or tray icon and open **Appearance** to choose a system, high-contrast dark, or high-contrast light palette; continuous or segmented bars; standard or slim bars; and standard or larger text. High-contrast palettes add a solid backdrop and bold text for readability over translucent taskbars. The **Recommended: translucent dark taskbar** action applies high-contrast dark colors, slim continuous rounded bars, and larger text. The widget also fits its height to the selected taskbar, avoiding clipped rows on 40-pixel taskbars. Appearance choices are saved in `settings.json`; existing settings keep their previous appearance until changed.

## Diagnostics

If you need to troubleshoot startup or visibility issues, run:

```powershell
codex-usage --diagnose
```

This writes a log file to:

```text
%TEMP%\codex-usage.log
```

The log records the application version, install channel, executable path, polling failure category, and retry timing. It does not log access tokens or credential contents. See [Troubleshooting](docs/troubleshooting.md) for the taskbar error labels and recovery steps.

Settings are saved to:

```text
%APPDATA%\CodexUsage\settings.json
```

## Account Support

Codex usage is read from the account authenticated in the local Codex installation. You can add more Codex logins as separate Codex CLI profiles; see [Multiple Codex Accounts](#multiple-codex-accounts). Optional Claude Code monitoring works with the account types supported by Claude Code.

As of **March 19, 2026**, Anthropic's Claude Code setup documentation says:

- **Supported:** Pro, Max, Teams, Enterprise, and Console accounts
- **Not supported:** the free Claude.ai plan

If Anthropic changes Claude Code availability in the future, this app should follow whatever Claude Code supports, as long as the usage data remains exposed through the same authenticated endpoints.

## Privacy And Security

This project is **open source**, so you can inspect exactly what it does.

What the app reads:

- Your local Claude Code OAuth credentials from `~/.claude/.credentials.json`
- If `CLAUDE_CONFIG_DIR` is set, the Claude Code credentials file in that directory
- If needed, the same credentials file inside an installed WSL distro
- If Codex is enabled, your local Codex credentials from `$CODEX_HOME/auth.json` or `~/.codex/auth.json`
- If Codex is enabled and extra logins are configured, the `auth.json` inside each `codex_home` listed in `codex_accounts`
- If Antigravity is enabled, your local Antigravity OAuth token from Windows Credential Manager target `gemini:antigravity`

What the app sends over the network:

- Requests to Anthropic's Claude endpoints to read your usage and rate-limit information
- Requests to ChatGPT's Codex usage endpoint to read your Codex usage and rate-limit information, if Codex is enabled
- Requests to Google's Cloud Code / Antigravity endpoints to read your Antigravity quota information, if Antigravity is enabled
- Requests to GitHub only if you use the app's update check / self-update feature
- If proxy environment variables such as `HTTPS_PROXY`, `HTTP_PROXY`, or `ALL_PROXY` are set, those outbound requests may use that proxy

What the app stores locally:

- Widget position
- Selected taskbar / screen
- Widget visibility
- Polling frequency
- Language preference
- Last update check time
- Visible quota rows and low-quota alert threshold
- Quota-window notification keys used to prevent duplicate alerts
- Displayed model preferences
- Configured Codex logins and which login the widget displays
- Appearance, selected display, left-edge pinning, and the faster-refresh setting

What it does **not** do:

- It does not send your credentials to any other server
- It does not use a separate backend service
- It does not collect analytics or telemetry
- It does not upload your project files
- It does not directly edit your Codex credentials file
- It does not read or reuse Claude Desktop authentication data

Notes:

- If your Claude Code token is expired, the app may ask the local Claude CLI to refresh it in the background
- If your Codex token is expired, the app may ask the local Codex CLI to refresh it in the background. The monitor does not write `auth.json` itself; any credential update is handled by the Codex CLI.
- **Add account…** only starts `codex login`; the browser sign-in and the resulting credentials file are handled entirely by the Codex CLI, and the monitor only reads the file afterwards.
- With more than one Codex login configured, the account email read from each login's stored ID token is shown in the tray summary, the Codex tooltip and the **Codex accounts** menu, so the logins can be told apart. A single-login install shows only `Codex`.
- If your Antigravity token is expired, open Antigravity and sign in again. The monitor does not write Windows Credential Manager entries itself.
- Portable installs can update themselves by downloading the latest release from this repository
- Proxies should be trusted because proxied usage requests include your OAuth bearer token inside the TLS connection

## How It Works

The monitor:

1. Finds your enabled model login credentials, including every configured Codex profile
2. Reads your current usage from Anthropic, ChatGPT, and/or Google's Antigravity endpoints
3. Shows the result directly in the Windows taskbar
4. Keeps the widget aligned with the selected taskbar and tray area
5. Refreshes periodically in the background

If the newer usage endpoint is unavailable, it can fall back to reading the rate-limit headers returned by Claude's Messages API.

## Open Source

This project is licensed under the MIT License. The original [LICENSE](LICENSE) and copyright notice are preserved.

The Claude and OpenAI marks come from [Lobe Icons](https://github.com/lobehub/lobe-icons) under the MIT License. See [src/icons/providers](src/icons/providers). Brand names and marks belong to their owners.

Codex Usage is a maintained derivative of [CodeZeno/Claude-Code-Usage-Monitor](https://github.com/CodeZeno/Claude-Code-Usage-Monitor). Thanks to Craig Constable and the upstream contributors for the original project. Changes in this repository are not affiliated with or endorsed by the upstream maintainers or OpenAI.

If you want to inspect the behavior or audit the code, everything is in this repository.
