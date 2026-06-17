// Base64 <-> bytes helpers for the PTY transport.
//
// PTY output arrives from Rust as a base64 string (one compact string instead
// of a JSON array of integers), and keystrokes are sent back the same way.
// These wrap the browser's atob/btoa, which only speak "binary strings".

/** Decode a base64 string into raw bytes. */
export function decodeBase64(b64: string): Uint8Array {
  const binary = atob(b64);
  const len = binary.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** Encode raw bytes into a base64 string. */
export function encodeBase64(bytes: Uint8Array): string {
  let binary = "";
  // Chunk to keep String.fromCharCode's argument count bounded — applying it
  // to a very large array at once can overflow the call stack.
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(binary);
}
