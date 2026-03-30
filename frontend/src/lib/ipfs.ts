const IPFS_GATEWAY =
  import.meta.env.VITE_IPFS_GATEWAY ?? "https://gateway.pinata.cloud";

/**
 * Fetch raw bytes from IPFS via the configured gateway.
 * Accepts a bare CID or a full ipfs:// URI.
 */
export async function fetchFromIpfs(cidOrUri: string): Promise<Uint8Array> {
  const cid = cidOrUri.replace(/^ipfs:\/\//, "").trim();
  const url = `${IPFS_GATEWAY}/ipfs/${cid}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`IPFS fetch failed: ${res.status} ${res.statusText}`);
  return new Uint8Array(await res.arrayBuffer());
}

/**
 * Decrypt AES-256-GCM ciphertext using a hex-encoded key.
 *
 * Expected ciphertext layout (produced by the seller's encryption tool):
 *   [12-byte IV][ciphertext + 16-byte auth tag]
 *
 * Returns the decrypted plaintext as a UTF-8 string when possible,
 * or a hex string for binary content.
 */
export async function decryptAesGcm(
  ciphertext: Uint8Array,
  hexKey: string
): Promise<string> {
  if (ciphertext.length < 13) {
    throw new Error("Ciphertext too short to contain IV.");
  }

  const keyBytes = hexToBytes(hexKey);
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    keyBytes.buffer as ArrayBuffer,
    { name: "AES-GCM" },
    false,
    ["decrypt"]
  );

  const iv = ciphertext.slice(0, 12);
  const data = ciphertext.slice(12);

  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    cryptoKey,
    data.buffer as ArrayBuffer
  );

  // Try UTF-8 decode; fall back to hex for binary blobs
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(plaintext);
  } catch {
    return bytesToHex(new Uint8Array(plaintext));
  }
}

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.replace(/^0x/, "");
  if (clean.length % 2 !== 0) throw new Error("Invalid hex key length.");
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
