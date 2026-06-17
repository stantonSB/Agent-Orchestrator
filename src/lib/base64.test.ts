import { describe, it, expect } from "vitest";
import { decodeBase64, encodeBase64 } from "./base64";

describe("base64 transport", () => {
  it("round-trips arbitrary bytes including control and high bytes", () => {
    const original = new Uint8Array([0x00, 0x1b, 0x5b, 0x41, 0x7f, 0xc3, 0xa9, 0x0d]);
    const encoded = encodeBase64(original);
    expect(decodeBase64(encoded)).toEqual(original);
  });

  it("round-trips an empty payload", () => {
    expect(encodeBase64(new Uint8Array([]))).toBe("");
    expect(decodeBase64("")).toEqual(new Uint8Array([]));
  });

  it("decodes a known base64 string to its bytes", () => {
    // "hi" === [0x68, 0x69] === "aGk="
    expect(decodeBase64("aGk=")).toEqual(new Uint8Array([0x68, 0x69]));
    expect(encodeBase64(new Uint8Array([0x68, 0x69]))).toBe("aGk=");
  });

  it("round-trips a large payload (exceeds the fromCharCode chunk size)", () => {
    const big = new Uint8Array(200_000);
    for (let i = 0; i < big.length; i++) big[i] = i % 256;
    expect(decodeBase64(encodeBase64(big))).toEqual(big);
  });

  it("matches a UTF-8 encoded string round-trip", () => {
    const text = "écho → 🚀\r\n";
    const bytes = new TextEncoder().encode(text);
    const decoded = decodeBase64(encodeBase64(bytes));
    expect(new TextDecoder().decode(decoded)).toBe(text);
  });
});
