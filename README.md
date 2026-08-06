<p align="center">
  <img src="icon.png" width="160" alt="Ichi Logo">
</p>

<h1 align="center">ICHI ・ イチ <sub><sup>for macOS</sup></sub></h1>

<p align="center">
  <strong>The definitive window snapping utility — ported to macOS.</strong><br>
  <em>Built in Rust for power, precision, and performance.</em>
</p>

<p align="center">
  <a href="https://github.com/mortenlein/ichi-mac/actions/workflows/ci.yml">
    <img src="https://github.com/mortenlein/ichi-mac/actions/workflows/ci.yml/badge.svg" alt="CI">
  </a>
  <a href="https://github.com/mortenlein/ichi-mac/releases/latest">
    <img src="https://img.shields.io/github/v/release/mortenlein/ichi-mac?style=flat&color=D0021B" alt="Latest Release">
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/github/license/mortenlein/ichi-mac?style=flat&color=333" alt="License">
  </a>
</p>

---

**Ichi** (イチ, meaning **"One"** or **"#1"**) is a high-performance, minimalist
window-snapping utility. This is the macOS port of
[the Windows original](https://github.com/mortenlein/ichi) — same snap geometry,
same hotkeys, same stay-out-of-the-way philosophy, rewritten against the
Accessibility and Carbon APIs instead of Win32.

## 🚀 Key Features

- **Identical geometry**: `engine.rs` is a straight port of the Windows snap
  math, so a window lands in exactly the same place on both platforms.
- **Stealth Mode**: runs as a background agent — no Dock icon, no Cmd-Tab entry,
  no window. It never steals focus.
- **Menu bar icon**: the hinomaru sits in the menu bar with a menu for settings,
  launch at login, and quit.
- **Configurable**: rebind every hotkey, set window gaps, and change the
  multi-tap ratios from a JSON settings file — no rebuild required.
- **Multi-Tap Cycling**: pressing the same hotkey repeatedly cycles through
  layout ratios (1/2, 1/3, 2/3).
- **Multi-display aware**: snaps within whichever display the window mostly
  occupies, respecting the menu bar and Dock.
- **Small**: ~430 KB stripped binary, nine direct dependencies.

## ⌨️ Global Hotkeys

All actions are triggered via **Ctrl + Option + Numpad** (Option is the key
labelled `alt`), matching the Windows build's Ctrl + Alt + Numpad:

| Hotkey | Action | Cycle Ratios |
| :--- | :--- | :--- |
| **Numpad 5** | Center / Maximize | 100% → 80% → 60% Center |
| **Numpad 4 / 6** | Left / Right Half | 1/2 → 1/3 → 2/3 Width |
| **Numpad 8 / 2** | Top / Bottom Half | 1/2 → 1/3 → 2/3 Height |
| **Numpad 7 / 9** | Top Left / Right | 1/2 → 1/3 → 2/3 Corner |
| **Numpad 1 / 3** | Bottom Left / Right | 1/2 → 1/3 → 2/3 Corner |

> **⚠️ The defaults need a numeric keypad.** They use the dedicated keypad key
> codes (`kVK_ANSI_Keypad1`–`9`), which a MacBook's built-in keyboard does not
> have — the number row sends different codes and will not trigger Ichi. On a
> laptop, rebind them in the [settings file](#️-settings); no rebuild needed.

## ⚙️ Settings

**Menu bar → Edit Settings…** opens `config.json`; **Restart to Apply Settings**
reloads it. Settings are read once at startup, so an edit needs that restart.

```
~/Library/Application Support/Ichi/config.json
```

Written with defaults on first launch. Every field is optional — anything
missing falls back to its default, so you can keep just the parts you change.

```json
{
  "hotkeys": {
    "top_left":     "ctrl+alt+keypad7",
    "top":          "ctrl+alt+keypad8",
    "top_right":    "ctrl+alt+keypad9",
    "left":         "ctrl+alt+keypad4",
    "center":       "ctrl+alt+keypad5",
    "right":        "ctrl+alt+keypad6",
    "bottom_left":  "ctrl+alt+keypad1",
    "bottom":       "ctrl+alt+keypad2",
    "bottom_right": "ctrl+alt+keypad3"
  },
  "gaps": { "screen_edge": 0, "between_windows": 0 },
  "cycle_ratios":  [0.5, 0.3333, 0.6667],
  "center_ratios": [1.0, 0.8, 0.6]
}
```

### Hotkeys

Modifiers plus one key, joined by `+`, case-insensitive. At least one modifier
is required — a bare key would be swallowed system-wide and you could no longer
type it.

| | |
| :--- | :--- |
| **Modifiers** | `ctrl`, `alt` (= `opt`, `option`), `cmd`, `shift` |
| **Keys** | `a`–`z`, `0`–`9`, `keypad0`–`keypad9`, `left`/`right`/`up`/`down`, `f1`–`f20`, `return`, `space`, `tab`, `escape`, `delete`, `home`, `end`, `pageup`, `pagedown`, and punctuation (`comma`, `period`, `slash`, `minus`, `equal`, `semicolon`, `quote`, `grave`, `backslash`, `leftbracket`, `rightbracket`) |

A laptop-friendly set, since the defaults need a keypad:

```json
"hotkeys": {
  "left":   "ctrl+alt+left",
  "right":  "ctrl+alt+right",
  "top":    "ctrl+alt+up",
  "bottom": "ctrl+alt+down",
  "center":       "ctrl+alt+return",
  "top_left":     "ctrl+alt+u",
  "top_right":    "ctrl+alt+i",
  "bottom_left":  "ctrl+alt+j",
  "bottom_right": "ctrl+alt+k"
}
```

A shortcut that will not parse, or that another app already owns, is skipped
with a warning in the menu bar and on stderr — the remaining bindings still
work, so one bad line never leaves you with nothing.

### Gaps

Both in points, both `0` by default (flush tiling, matching Windows).

| Field | Effect |
| :--- | :--- |
| `screen_edge` | Margin between snapped windows and the edge of the usable screen |
| `between_windows` | Channel between two adjacent windows; each gives up half |

The two never compound: an edge touching the screen boundary only gets
`screen_edge`, so the outer margin stays exactly what you asked for.

### Cycle ratios

`cycle_ratios` drives halves and corners, `center_ratios` the centre position.
Any length — three entries cycle every three taps, one entry disables cycling.
Values must be in `(0.0, 1.0]`; anything else is dropped, and if that empties
the list the defaults come back.

### Launch at login

A menu bar toggle, backed by `SMAppService`. It registers the app bundle, so it
only works from `/Applications` — under `cargo run` the item is disabled. macOS
may also ask you to approve Ichi in **System Settings → General → Login Items**,
which the menu says when that is the case.

## 🏗️ Architecture

The port replaces every OS-facing call; the geometry engine is untouched.

| Concern | Windows | macOS (this build) |
| :--- | :--- | :--- |
| Global hotkeys | `WH_KEYBOARD_LL` hook | `RegisterEventHotKey` (Carbon) |
| Move / resize | `SetWindowPos` | Accessibility API (`AXUIElement`) |
| Work area | `GetMonitorInfoW` | `NSScreen.visibleFrame` |
| Flicker control | `WM_SETREDRAW` | not needed — Quartz composites |
| Frame correction | `DWMWA_EXTENDED_FRAME_BOUNDS` | not needed — AX frames are exact |

Two details are worth knowing:

1. **Coordinate flip.** AppKit measures from the bottom-left with Y increasing
   upward; the Accessibility API measures from the top-left with Y increasing
   downward — the same convention as Win32. `window.rs` converts `NSScreen`
   rects into AX space, which is why the shared engine needs no changes.

2. **`RegisterEventHotKey` over an event tap.** It needs no Accessibility
   permission of its own and only ever sees the nine registered combinations,
   rather than the whole keystroke stream. The system also consumes a matched
   hotkey before the focused app sees it — the same effect the Windows build
   gets by returning `LRESULT(1)` from its hook.

### Inherited rounding

On a work area with an odd height, the top and bottom halves each truncate to
the same integer and leave a one-pixel seam (e.g. 1055 ÷ 2 → 527 + 527). This
comes from the original Windows engine and is reproduced deliberately so both
builds place windows identically. See
`engine::tests::vertical_halves_respect_the_menu_bar_inset`.

## 🛠️ Building & Installation

### Prerequisites
- [Rust Toolchain](https://rustup.rs/) (Stable)
- Xcode Command Line Tools (`xcode-select --install`)
- macOS 13 or later (`SMAppService`, which backs launch at login)

### Build

```bash
./bundle.sh --install
```

That builds, installs to `/Applications`, stops any running copy, and relaunches.
The stop matters: Ichi is a background agent with no window, so re-launching
over a running instance silently does nothing and you end up testing the old
build.

### Grant Accessibility access

Moving another application's windows requires it. On first launch Ichi raises
the system prompt; otherwise:

**System Settings → Privacy & Security → Accessibility** → enable **Ichi**.

Then **relaunch Ichi** — macOS only applies the grant to a freshly started
process.

> `bundle.sh` ad-hoc signs the app (`codesign --sign -`). That is enough for
> the permission to stick across rebuilds on *this* machine. To share the app
> with anyone else, substitute a Developer ID and notarize it.

### Run at login

**System Settings → General → Login Items → Open at Login → +** and pick
`/Applications/Ichi.app`.

### Run from source (development)

```bash
cargo run --release
```

Note that a bare binary launched from a terminal inherits the *terminal's*
Accessibility permission, so snapping will only work if your terminal has been
granted access. The bundle is the supported path.

## 📦 Releases

Two workflows run on GitHub Actions:

| Workflow | Trigger | Does |
| :--- | :--- | :--- |
| `ci.yml` | push to `main`, PRs | `cargo fmt --check`, clippy with `-D warnings`, tests, and a bundling smoke test |
| `release.yml` | tag `v*`, or manual dispatch | builds a universal binary, packages `.dmg` + `.zip`, opens a **draft** release |

Cutting a release mirrors the Windows repo — bump `version` in `Cargo.toml`, then:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

The release is left as a draft so you can review the artifacts before
publishing.

### Building release artifacts locally

```bash
./bundle.sh --universal --dmg
```

Produces `target/Ichi-<version>.dmg` and `target/Ichi-<version>-macos.zip`.
`package.sh` handles the packaging step on its own, which is what lets the
release workflow sign, notarize, and staple the app *before* it is wrapped up.

### ⚠️ Gatekeeper and code signing

By default both the local build and CI sign **ad-hoc**, which is only trusted on
the machine that produced it. A downloaded ad-hoc build is quarantined, and
macOS refuses to open it — usually claiming the app is "damaged". The escape
hatch is:

```bash
xattr -dr com.apple.quarantine /Applications/Ichi.app
```

That is fine for your own machine, but it is not something to ask other people
to do. To ship properly, add these repository secrets and the release workflow
will sign with your Developer ID and notarize with Apple automatically:

| Secret | What it is |
| :--- | :--- |
| `MACOS_CERTIFICATE` | Developer ID Application cert, exported as `.p12`, base64-encoded |
| `MACOS_CERTIFICATE_PWD` | Password used when exporting that `.p12` |
| `MACOS_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Apple ID used for notarization |
| `APPLE_TEAM_ID` | Your 10-character team ID |
| `APPLE_APP_PASSWORD` | An [app-specific password](https://support.apple.com/en-us/102654), not your Apple ID password |

With `MACOS_CERTIFICATE` absent the workflow still succeeds — it warns, and adds
the quarantine instructions to the release notes. Signing locally works the same
way:

```bash
SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./bundle.sh --universal
```

Note that a Developer ID requires a paid Apple Developer Program membership.

### Tests

```bash
cargo test
```

50 tests cover the snap geometry (grid positions, cycle ratios, work-area
offsets, multi-display selection), gaps, shortcut parsing, config loading and
sanitising, the AppKit→AX coordinate flip, the Carbon four-character-code and
key-code constants, and the embedded menu bar asset.

### Regenerating the menu bar icon

`src/menubar-icon.png` is derived from `icon.png` and embedded in the binary
via `include_bytes!`, so it works identically under `cargo run` and in the
bundle. The app icon's white rounded square would read as a bright blob in the
menu bar, so the generator finds the red circle, crops to it, and clips to an
ellipse — leaving the surround transparent and the white イチ intact.

After changing `icon.png`:

```bash
swift make_menubar_icon.swift
```

## 📄 License

MIT — see [LICENSE](LICENSE).
