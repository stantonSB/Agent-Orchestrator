// A single 1 Hz ticker shared by every DurationTimer, replacing one
// `setInterval` per running session. The interval only runs while at least one
// subscriber is active, and stops automatically when the last one unsubscribes.
//
// Designed for React's `useSyncExternalStore`: `subscribeNow` registers a
// listener and returns an unsubscribe; `getNow` reads the latest tick.

let now = Date.now();
const listeners = new Set<() => void>();
let intervalId: ReturnType<typeof setInterval> | null = null;

function tick() {
  now = Date.now();
  for (const listener of listeners) listener();
}

export function subscribeNow(listener: () => void): () => void {
  listeners.add(listener);
  if (intervalId === null) {
    // Snap to the current time so a freshly-mounted timer is accurate before
    // the first interval fires.
    now = Date.now();
    intervalId = setInterval(tick, 1000);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && intervalId !== null) {
      clearInterval(intervalId);
      intervalId = null;
    }
  };
}

export function getNow(): number {
  return now;
}
