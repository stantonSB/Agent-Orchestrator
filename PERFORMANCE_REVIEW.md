# Agent Orchestrator — Performance Review

**Date:** 2026-06-17
**Scope:** Slowdown observed when several sessions run concurrently.
**Method:** Static read of the PTY output → IPC → xterm render pipeline and the
React/Zustand re-render graph. No code changed; this is an analysis + remediation plan.

---

## TL;DR

The app slows down with many concurrent sessions because of **three compounding
costs on the terminal-output hot path**, each multiplied by the number of active
sessions:

1. **PTY output is shipped to the UI as a JSON array of integers** (`data: number[]`).
   Every byte of terminal output is serialized as `[27,91,51,...]` — roughly **3–4× payload
   bloat** plus expensive JSON number parsing on the JS side, for every chunk of every session.
2. **No output batching.** The reader thread emits one Tauri event per `read()` (≤4 KB).
   Bursty output (Claude streaming, build logs) becomes a flood of tiny events; with N
   sessions the event rate multiplies.
3. **xterm.js runs on the default DOM renderer.** No WebGL/canvas addon is loaded, so
   high-throughput output with a 10k-line scrollback thrashes layout on the main thread.

These three sit in series on the same path, so their costs multiply. Fixing #1 and #3
are the highest-leverage changes; #2 reduces fixed per-event overhead across all sessions.

A second tier of issues causes **whole-tree React re-renders on every status change**
(unstable ref callbacks + missing memoization), which gets worse linearly with session count.

---

## Hot path (where the time goes)

```
Claude/shell child
   │  raw bytes
   ▼
pty_manager reader thread        buf[4096], one emit per read()      ← H2: no batching
   │  Vec<u8>
   ▼
lib.rs on_output
   json!({ "data": data })       Vec<u8> → JSON array of ints        ← H1: 3–4× bloat
   handle.emit(event_name, …)
   │  IPC bridge (JSON string)
   ▼
TerminalArea listener
   new Uint8Array(payload.data)  parse number[] → typed array        ← H1 (JS side)
   handle.write(bytes)
   │
   ▼
xterm.js                         DOM renderer, 10k scrollback        ← H3: no GPU renderer
```

Everything on this path runs **per session, per chunk**. With several busy sessions it is
the bottleneck.

---

## Findings

### HIGH IMPACT

#### H1 — PTY output serialized as a JSON integer array
**`src-tauri/src/lib.rs:94-101`**, consumed at **`src/components/TerminalArea/TerminalArea.tsx:93-105`**, typed at **`src/types/tauri-events.ts` (`SessionOutputPayload.data: number[]`)**.

```rust
let on_output = Box::new(move |id, data| {            // data: Vec<u8>
    let event_name = format!("session-output-{}", id);
    let _ = handle_for_output.emit(&event_name, serde_json::json!({ "data": data }));
});
```

`serde` serializes `Vec<u8>` as a JSON array of numbers, so a 4 KB chunk of terminal
output becomes a `[27,91,51,...]` string of ~12–16 KB. That string is built on the Rust
side, pushed through the IPC bridge, then parsed back into a JS `number[]` and finally
copied into a `Uint8Array` (`new Uint8Array(payload.data)`). Per chunk, per session.

**Fix (preferred):** stream PTY output over a **Tauri 2 `Channel<&[u8]>`** instead of an
event. Channels carry raw bytes with no JSON encoding — the frontend receives an
`ArrayBuffer`/`Uint8Array` directly. Create one channel per session at `create_session`
time and hand it to the reader.

**Fix (lighter touch):** keep events but **base64-encode** the bytes (`emit(&name, b64_string)`)
and `atob`-decode on the frontend. ~5.4 KB string for a 4 KB chunk vs ~16 KB, a single JSON
string instead of a 4096-element array, and a fast decode. Smaller diff than channels.

**Expected gain:** large. Removes the dominant per-byte serialization/parse cost on the
busiest path.

---

#### H2 — No output coalescing in the reader thread
**`src-tauri/src/pty_manager.rs:490-511`**

```rust
let mut buf = [0u8; 4096];
loop {
    match reader.read(&mut buf) {
        Ok(0) => break,
        Ok(n) => { let data = buf[..n].to_vec(); cb(reader_id.clone(), data); } // emit per read
        ...
    }
}
```

Every `read()` triggers a full emit → serialize → IPC → listener dispatch → `xterm.write`.
A program that prints rapidly produces many small reads, each paying that fixed overhead.
With several sessions, the aggregate event rate is what saturates the bridge.

**Fix:** coalesce in the reader thread. Accumulate into a `Vec<u8>` and flush when either
a size threshold (e.g. 32–64 KB) or a short time window (e.g. 4–8 ms) is reached, then emit
one larger chunk. xterm.js handles large writes efficiently; the win is fewer round-trips.
Pairs naturally with H1 (fewer, bigger payloads).

**Expected gain:** medium–high under bursty/concurrent load; reduces fixed per-event cost.

---

#### H3 — xterm.js uses the default DOM renderer (no GPU acceleration)
**`src/components/XTermInstance/useTerminal.ts:103-124`**, **`package.json:24-28`**

Loaded addons: `fit`, `search`, `unicode-graphemes`, `web-links`. No `@xterm/addon-webgl`
or `@xterm/addon-canvas`. The DOM renderer is the slowest xterm rendering path; combined
with `scrollback: 10_000` and high-throughput output it does heavy layout work on the main
thread, which is shared by every session and all React rendering.

**Fix:** add `@xterm/addon-webgl` and load it after `term.open()`, with a graceful fallback
to canvas/DOM if WebGL context creation fails or the `webglcontextlost` event fires. The
WebGL renderer also naturally does less work when its canvas isn't visible, which helps the
"many background terminals" case (see M1).

**Expected gain:** large for render-bound throughput; frees the main thread for input latency
and React.

---

### MEDIUM IMPACT

#### M1 — Hidden (inactive) terminals still parse and render output
**`src/components/TerminalArea/TerminalArea.tsx:312-326`**, **`src/components/XTermInstance/XTermInstance.tsx:105`**

By design all terminals stay mounted (CSS `display:none` for inactive) to preserve
scrollback. Output for background sessions is still `write()`-ten and the renderer still
runs. With the DOM renderer this is wasted layout work for terminals the user can't see.
Adopting WebGL (H3) reduces this because the renderer does little when its canvas is not
visible; for extra savings the renderer can be paused/refreshed on `isActive` transitions.
Note: the *parse* into the buffer must still happen to keep scrollback correct — only the
*render* is safe to skip while hidden.

**Expected gain:** medium, scales with number of busy background sessions.

---

#### M2 — Unstable `ref` callbacks force detach/reattach on every render
**`src/components/TerminalArea/TerminalArea.tsx:62-77, 312-326`**

```tsx
const setRef = useCallback((id) => (handle) => { /* … */ }, []);
...
<XTermInstance ref={setRef(session.id)} … />   // new function identity every render
```

`setRef(session.id)` returns a **new function instance on every render**. React treats a
new ref callback as a change: it invokes the old ref with `null` then the new ref with the
handle, for **every terminal on every `TerminalArea` re-render** — and `TerminalArea`
re-renders on every session status change (see M3). This churns `refsMap` and re-runs the
buffered-output flush check for all sessions.

**Fix:** cache one stable ref callback per session id (e.g. a `useRef(new Map())` of
id → callback), so identity is stable across renders.

**Expected gain:** medium; removes per-render ref churn that scales with session count.

---

#### M3 — Status changes trigger a whole-tree re-render; key components aren't memoized
**`src/stores/sessionStore.ts:171-179`**, **`src/App.tsx:79-83, 161-175`**, **`SessionCard.tsx`**, **`XTermInstance.tsx`**

Every status event clones the `sessions` Map (`updateSessionStatus`). `App` subscribes to
`sessions`, so it re-renders, recomputes `sessionList`/`orderedSessionIds`, and passes a new
array to both `TerminalArea` and `SessionPanel`. Because `SessionCard`, `XTermInstance`, and
`ProjectGroup` are **not** wrapped in `React.memo`, every card and every terminal component
reconciles on each status tick. The fan-out grows with session count.

**Fix:** wrap `SessionCard`, `XTermInstance`, and `ProjectGroup` in `React.memo`; ensure the
props they receive are referentially stable (stable callbacks via `useCallback`, and the
per-id ref fix from M2). Optionally have `SessionCard` subscribe to its own session slice so a
single session's status change doesn't re-render siblings.

**Expected gain:** medium; converts an O(N) re-render per status event into O(1) for the
changed card.

---

#### M4 — `cursorBlink: true` on every terminal, including hidden ones
**`src/components/XTermInstance/useTerminal.ts:108`**

Each terminal runs a cursor-blink timer that repaints the cursor cell ~every 0.5–0.6 s,
even while `display:none`. With many sessions this is N timers and N periodic repaints for
cursors nobody is looking at.

**Fix:** disable blink globally, or toggle `term.options.cursorBlink` so only the active
terminal blinks. Small but free win, and it compounds with M1.

**Expected gain:** low–medium with many sessions.

---

### LOW IMPACT

#### L1 — One 1 s interval per running session for the duration timer
**`src/components/DurationTimer/DurationTimer.tsx`**

Each running session mounts its own `setInterval(…, 1000)`. The re-render is local to the
timer, so it's cheap, but it's still N timers and N reconciles per second.

**Fix (optional):** a single shared 1 Hz ticker (context or store field) that all timers read,
instead of one interval each.

**Expected gain:** low.

---

#### L2 — Keystroke writes also use the `number[]` encoding
**`src/components/TerminalArea/TerminalArea.tsx:205-215`**, **`src/lib/tauri-ipc.ts:29-32`**

`writeToSession` sends `data: number[]`. This is the same inefficiency as H1 but on the
write path, which is keystroke-frequency (low volume), so the impact is small. Worth
unifying onto whatever transport H1 adopts (channel/base64) for consistency.

**Expected gain:** low.

---

## Recommended order of work

1. **H3 — add `@xterm/addon-webgl`.** Smallest diff, large render win, low risk (with fallback).
2. **H1 — switch PTY output to a binary `Channel` (or base64).** Removes the dominant
   serialization cost; biggest single improvement.
3. **H2 — coalesce reader output** (size/time window). Compounds with H1.
4. **M2 + M3 — stable ref callbacks + `React.memo`.** Stops O(N) re-renders per status tick.
5. **M1 / M4 / L1 / L2 — opportunistic** once the above land.

Items 1–3 are independent and can be done in parallel. Items 4 are pure-frontend and low risk.

---

## How to verify (before/after)

- **Throughput test:** in one session run a high-volume printer (`yes "$(seq 1 200)"` or
  `find / 2>/dev/null`) and watch CPU + UI responsiveness; repeat with 4–6 sessions doing it
  at once. Compare main-thread time in the webview devtools Performance panel.
- **Event volume:** temporarily count `session-output-*` emits/sec (log in `on_output`)
  before and after H2 to confirm coalescing reduces event rate.
- **Payload size:** log `data.len()` vs serialized length before/after H1 to confirm the
  bloat is gone.
- **Re-render count:** React DevTools "Highlight updates" while a single session changes
  status — before M2/M3 every card/terminal flashes; after, only the changed card should.
- **Renderer:** confirm WebGL is active (no `webglcontextlost` fallback) and frame times
  drop during heavy output.

No regression in correctness is expected: scrollback (10k), search, unicode, links, and
status hooks are all orthogonal to these changes. Keep the reader's parse-into-buffer
behavior intact when skipping *render* for hidden terminals (M1).
