# Telegram Telephony Sidecar Architecture & Implementation Notes

## 1. Overview
A lightweight Python sidecar service enabling bidirectional 1-on-1 and group voice calling over Telegram, connected to the existing Rust `voice-assistant` backend via local WebSockets (`ws://127.0.0.1:8080/ws`).

This provides a **100% free**, zero-carrier-fee VoIP telephony interface for the voice agent that anyone with Telegram on their smartphone or desktop can call.

---

## 2. High-Level Architecture

```
[ Telegram App on User Phone / PC ]
               │
               ▼ (Telegram VoIP: 48kHz Opus over WebRTC / MTProto)
┌────────────────────────────────────────────────────────┐
│  Python Telephony Sidecar (Telethon/Pyrogram + tgcalls)│
│                                                        │
│  1. Listens for incoming calls or initiates calls      │
│  2. WebRTC Audio Stream Sink / Source                  │
│  3. Resamples audio:                                   │
│     - Inbound:  48kHz Opus -> 16kHz PCM16              │
│     - Outbound: 16kHz PCM16 -> 48kHz Opus              │
│  4. Manages WebSocket client to Rust backend           │
└───────────────────────┬────────────────────────────────┘
                        │
                        ▼ (Localhost WebSocket: 16kHz PCM)
┌────────────────────────────────────────────────────────┐
│  Rust Voice Assistant Backend                          │
│                                                        │
│  1. Silero VAD with speech confirmation (debounce)     │
│  2. Streaming STT (Azure / Deepgram WebSocket)         │
│  3. Agent Loop (OpenAI + Tools like Weather & Search)  │
│  4. TTS Engine (Azure SSML chunked MP3/PCM audio)      │
└────────────────────────────────────────────────────────┘
```

---

## 3. Technology Stack

* **Language / Runtime:** Python 3.10+
* **Telegram MTProto Client:** `Telethon` or `Pyrogram`
* **VoIP Engine:** `py-tgcalls` (backed by the native C++ `ntgcalls` engine)
* **Audio Resampling:** `soxr`, `scipy.signal`, or `pydub`
* **Transport:** `websockets` (standard async Python WebSocket client)

---

## 4. Crucial Details & Constraints

### A. Telegram User Account Required (Userbot)
* **The Constraint:** Telegram's official Bot API (`@BotFather` bot tokens) **does not support 1-on-1 private VoIP calls**. Official bots can only join Group Voice Chats.
* **The Solution:** The Python sidecar must run as a **Telegram User session** (a "Userbot") using an `api_id` and `api_hash` generated from [my.telegram.org](https://my.telegram.org) and authenticated with a dedicated phone number (e.g. a secondary SIM card or virtual number).

### B. Audio Codec & Sample Rate Conversion
* **Telegram WebRTC:** Operates natively at **48,000 Hz (48kHz) Opus**.
* **Rust Backend (Silero VAD & STT):** Operates at **16,000 Hz (16kHz) 16-bit PCM (PCM16)**.
* **Pipeline:**
  * **Incoming (User speaking):** Decode Opus 48kHz -> Downsample to 16kHz PCM16 -> Send raw bytes to Rust WS.
  * **Outgoing (AI speaking):** Receive audio chunks from Rust WS -> Upsample to 48kHz Opus -> Play into `py-tgcalls` stream.

### C. Inbound vs Outbound Calling Rules
* **Inbound Calls (Caller dials Assistant):** Works reliably for anyone who can see the userbot's Telegram handle or phone number.
* **Outbound Calls (Assistant dials Caller):** To avoid Telegram's automated spam detection and account bans:
  * The user must either have the bot's phone number saved in their phone contacts, OR
  * The user must have initiated a text message chat with the bot account first.

---

## 5. Implementation Roadmap

1. **Prerequisite:** Finalize Streaming STT in the Rust backend to minimize turnaround latency.
2. **Setup Sidecar Directory:**
   * Create `telephony-sidecar/` with `requirements.txt` (`telethon`, `py-tgcalls`, `websockets`, `soxr`).
3. **Session Authentication:**
   * Create a one-time session login script to save `assistant_userbot.session`.
4. **Call Handler Implementation:**
   * Register incoming call event hooks using `py-tgcalls`.
   * Open WebSocket connection to `ws://127.0.0.1:8080/ws` upon call connection.
   * Pipe audio bi-directionally between `py-tgcalls` raw streams and the WebSocket.
5. **Call Lifecycle & Termination:**
   * Close WebSocket and abort active AI turn when either party hangs up.
