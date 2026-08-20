# Bornika: Bangla Phonetic IME for Linux (IBus)

Bornika is a lightweight, secure, pure-Rust background Input Method Editor (IME) for Linux. Designed natively for modern desktop systems (like Fedora running Wayland and GNOME), it allows typing Bangla phonetically using a standard QWERTY layout.

By connecting directly to the **IBus (Intelligent Input Bus)** framework over the system's private D-Bus socket, Bornika runs entirely in user space without requiring root privileges or installing heavy C development libraries.

---

## Key Features

* **Complete Avro Phonetic Support:** Fully supports all standard phonetic combinations (e.g., `kotha` $\rightarrow$ `কথা`, `kOtha` $\rightarrow$ `কোথা`, `rri` $\rightarrow$ `ঋ`, `S` $\rightarrow$ `শ`, `Sh` $\rightarrow$ `ষ`, `borrd` $\rightarrow$ `বর্ড`).
* **Wayland & X11 Native:** Native compatibility across all window servers by leveraging the IBus system service.
* **Pure Rust Architecture:** Built using `tokio` and `zbus` to connect directly over D-Bus. No dynamic link bindings (`libibus-devel`, `glib2-devel`) are required to compile.
* **On-the-Fly Toggle:** Press **`Ctrl + Space`** within any active text input to toggle between English and Bangla phonetic modes.
* **Real-time Composition Styling:** Displays uncommitted text inline with a composition underline, guaranteeing native visual feedback in modern editors (like VS Code, Chrome, Firefox, and GTK text fields).
* **Shortcut & Control Pass-through:** Standard layout operations (like `Ctrl + C`, `Ctrl + V`, `Ctrl + A`, `Space`, `Enter`) bypass phonetic interception automatically.

---

## Phonetic Layout Guide

Bornika follows the standard Avro phonetic transliteration guidelines:

### Vowels & Diacritics (Kar / Matra)
| Key | Independent | Dependent | Example |
|---|---|---|---|
| `o` | অ | | `kotha` $\rightarrow$ `কথা` |
| `O` | ও | ো | `kOtha` $\rightarrow$ `কোথা` |
| `a` | আ | া | `amar` $\rightarrow$ `আমার` |
| `i` | ই | ি | `iti` $\rightarrow$ `ইতি` |
| `I` / `ee` | ঈ | ী | `kee` $\rightarrow$ `কী` |
| `u` / `oo` | উ | ু | `kuku` $\rightarrow$ `কুকু` / `koo` $\rightarrow$ `কু` |
| `U` | ঊ | ূ | `dUro` $\rightarrow$ `দূর` |
| `e` | এ | ে | `keno` $\rightarrow$ `কেন` |
| `oi` / `OI` | ঐ | ৈ | `kOI` $\rightarrow$ `কৈ` |
| `ou` / `OU` | ঔ | ৌ | `kOU` $\rightarrow$ `কৌ` |
| `rri` | ঋ | ৃ | `krriho` $\rightarrow$ `গৃহ` |

### Consonants & Signs
| Key | Bengali | Key | Bengali | Key | Bengali |
|---|---|---|---|---|---|
| `k` / `ko` | ক | `kh` | খ | `g` | গ |
| `gh` | ঘ | `c` | চ | `ch` | ছ |
| `j` | জ | `jh` | ঝ | `T` | ট |
| `Th` | ঠ | `D` | ড | `Dh` | ঢ |
| `N` | ণ | `t` | ত | `th` | থ |
| `d` | দ | `dh` | ধ | `n` | ন |
| `p` | প | `ph` / `f` | ফ | `b` | ব |
| `bh` / `v` | ভ | `m` | ম | `z` / `Z` | য / য-ফলা |
| `r` | র | `l` | ল | `S` / `sh` | শ |
| `Sh` | ষ | `s` | স | `h` | হ |
| `R` | ড় | `Rh` | ঢ় | `y` / `Y` | য় / য-ফলা |
| `Ng` / `NG` | ঙ / ঞ | `ng` | ং | `:` | ঃ |
| `^` | ঁ | `x` / `kx` | ক্স / ক্ষ | `J` | জ় |
| `t`` ` | ৎ | `$` | ৳ | | |

---

## Installation & Setup

### Requirements
* Fedora (or any Linux distribution running IBus).

### Quick Install (Pre-built Release)
To download and install the latest pre-built release of Bornika directly on your system, run:

```bash
curl -fsSL https://raw.githubusercontent.com/itsfuad/Bornika/main/install.sh | sh
```

This script automatically:
1. Downloads the latest pre-built `bornika-daemon` binary from GitHub Releases.
2. Deploys the background binary to `~/.local/bin/bornika-daemon`.
3. Registers the Bornika engine template at `/usr/share/ibus/component/bornika.xml` (requires one-time `sudo` authentication to copy to the system directory).
4. Restarts the active IBus daemon session to load the engine.

### Building from Source
If you prefer to compile and install Bornika from source, you will need the Rust toolchain (Cargo & Rustc) installed. Clone the repository and run:

```bash
./build.sh
```

---

## System Activation

Once installed, register Bornika as an active Input Source in your desktop environment:

1. Open your system's **Settings** (e.g. GNOME Settings).
2. Navigate to **Keyboard** $\rightarrow$ **Input Sources**.
3. Click the **`+` (Add)** button.
4. Select **Bengali** $\rightarrow$ **Bengali (Bornika)** and click **Add**.
5. Switch to the Bornika input source from your top-bar menu or press **`Super + Space`**.
6. Type **`Ctrl + Space`** within any text input box to toggle between English and Bangla modes.

---

> A tribute to "Mehdi Hasan Khan" - The creator of Avro Keyboard.

## License

Bornika is open-source software. Feel free to use, modify, and distribute it under the terms of the project's license.
