# Security & trust

Steno asks for three powerful Windows capabilities — microphone access, system-wide keyboard
simulation, and a global hotkey listener. These are the same capabilities a keylogger or
audio-surveillance tool uses, so it's worth being explicit about what Steno does and doesn't do.

## What Steno does (and only does)

- Captures microphone audio while you're holding `Shift+Space`.
- Resamples it to 16 kHz in memory and runs it through the local Whisper transcription model
  (file: `models/ggml-base.en-q5_1.bin`).
- Types the transcribed text into whatever window currently has keyboard focus, by simulating
  keystrokes.
- Shows recent transcriptions in the Steno window with a copy button.

## What Steno doesn't do

- **No network calls.** Steno makes zero outbound HTTP requests. No telemetry. No auto-update
  check. No phone-home. Run a network monitor (e.g. Wireshark, Fiddler, `netstat`) while using
  it — you'll see nothing from Steno.
- **No disk persistence of audio.** The audio buffer lives in memory only and is cleared at
  the start of every utterance. Nothing is written to disk.
- **No background recording.** The microphone is only active while `Shift+Space` is held down.
  Release the keys and capture stops.
- **No account, no API key, no login, no cloud anything.** There's nothing to sign into.

You can verify each of these by reading the source — it's small (a few hundred lines of Rust)
and well-commented.

## Why Windows SmartScreen flags Steno

The first time you run the downloaded `.exe`, Windows shows:

> Microsoft Defender SmartScreen prevented an unrecognized app from starting

This is **not** a virus detection. It's a "Windows hasn't seen this binary before" warning
that fires on every unsigned executable downloaded from the internet — including legitimate
indie software. The fix is a code-signing certificate (~$200/year) or distribution through
the Microsoft Store ($19 one-time), neither of which this project has yet.

To run Steno the first time:

1. Click **More info** in the SmartScreen dialog.
2. Click **Run anyway**.

After that first run Windows won't ask again for this binary.

## Verifying your download

Every release includes a `SHA256SUMS` file. Verify yours with PowerShell:

```powershell
Get-FileHash -Algorithm SHA256 steno-0.1.0-windows-x64.zip
```

The output should match the corresponding entry in `SHA256SUMS`.

## Verifying the source matches the binary

Don't trust the binary? **Build it yourself.** The README's *Build from source* section walks
through the full setup. The released binary is built from the matching git tag with no extra
steps. If your local build behaves differently from the released binary, please open an issue
— that's exactly the kind of mismatch this section exists to catch.

## One real warning for users

Steno types into whatever window currently has keyboard focus. If you hold `Shift+Space`
while a **password field** is focused, your transcription will be typed *into that field*.
Same goes for any sensitive field. The hotkey is push-to-talk by design — only what you say
while holding the keys gets typed — but be aware of which window is focused before you start.

## Reporting a security issue

Open a GitHub issue, or email [TBD — add a contact address once the repo is public].
