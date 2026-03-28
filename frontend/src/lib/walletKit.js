/**
 * walletKit.js
 *
 * Thin abstraction over Freighter, xBull, and Lobstr wallets.
 * Mirrors the interface that contractClient.js expects:
 *   wallet.address          — Stellar public key (G...)
 *   wallet.signTransaction  — (xdr: string) => Promise<string>  (returns signed XDR)
 *
 * localStorage key for persistence: "swk_wallet_id"
 */

export const WALLET_IDS = {
  FREIGHTER: "freighter",
  XBULL: "xbull",
  LOBSTR: "lobstr",
};

const STORAGE_KEY = "swk_wallet_id";

// ─── Freighter ────────────────────────────────────────────────────────────────

async function freighterGetAddress() {
  const { getPublicKey, isConnected } = await import("@stellar/freighter-api");
  const connected = await isConnected();
  if (!connected) throw new Error("Freighter is not installed or not connected.");
  return getPublicKey();
}

async function freighterSign(xdr, networkPassphrase) {
  const { signTransaction } = await import("@stellar/freighter-api");
  const result = await signTransaction(xdr, { networkPassphrase });
  // freighter-api v3 returns { signedTxXdr } or the raw string depending on version
  return typeof result === "string" ? result : result.signedTxXdr;
}

// ─── xBull ────────────────────────────────────────────────────────────────────

async function xbullGetAddress() {
  if (!window.xBullSDK && !window.xbull) {
    throw new Error("xBull wallet is not installed.");
  }
  // xBull injects window.xBullSDK or can be used via the npm package
  const { xBullWalletConnect } = await import("@creit.tech/xbull-wallet-connect");
  const connection = new xBullWalletConnect();
  const { publicKey } = await connection.connect({ canRequestPublicKey: true, canRequestSign: true });
  return publicKey;
}

async function xbullSign(xdr, networkPassphrase) {
  const { xBullWalletConnect } = await import("@creit.tech/xbull-wallet-connect");
  const connection = new xBullWalletConnect();
  const { signedXDR } = await connection.sign({ xdr, publicKey: undefined, network: networkPassphrase });
  return signedXDR;
}

// ─── Lobstr ───────────────────────────────────────────────────────────────────

async function lobstrGetAddress() {
  if (!window.lobstr) {
    throw new Error("Lobstr Signer Extension is not installed.");
  }
  return window.lobstr.getPublicKey();
}

async function lobstrSign(xdr, networkPassphrase) {
  if (!window.lobstr) {
    throw new Error("Lobstr Signer Extension is not installed.");
  }
  const result = await window.lobstr.signTransaction(xdr, { networkPassphrase });
  return typeof result === "string" ? result : result.signedXdr ?? result.signedTxXdr;
}

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Returns true if the given wallet extension appears to be installed.
 * This is a best-effort check; some wallets only reveal themselves after user interaction.
 */
export function isWalletAvailable(walletId) {
  switch (walletId) {
    case WALLET_IDS.FREIGHTER:
      return typeof window !== "undefined" && !!window.freighter;
    case WALLET_IDS.XBULL:
      return typeof window !== "undefined" && (!!window.xBullSDK || !!window.xbull);
    case WALLET_IDS.LOBSTR:
      return typeof window !== "undefined" && !!window.lobstr;
    default:
      return false;
  }
}

/**
 * Connect to a wallet by ID. Returns a wallet object compatible with contractClient.js.
 * @param {string} walletId — one of WALLET_IDS
 * @param {string} networkPassphrase
 * @returns {Promise<{ address: string, walletId: string, signTransaction: (xdr: string) => Promise<string> }>}
 */
export async function connectWallet(walletId, networkPassphrase) {
  let address;

  switch (walletId) {
    case WALLET_IDS.FREIGHTER:
      address = await freighterGetAddress();
      break;
    case WALLET_IDS.XBULL:
      address = await xbullGetAddress();
      break;
    case WALLET_IDS.LOBSTR:
      address = await lobstrGetAddress();
      break;
    default:
      throw new Error(`Unknown wallet: ${walletId}`);
  }

  localStorage.setItem(STORAGE_KEY, walletId);

  return {
    address,
    walletId,
    signTransaction: (xdr) => signWithWallet(walletId, xdr, networkPassphrase),
  };
}

/**
 * Sign a transaction XDR with the specified wallet.
 */
async function signWithWallet(walletId, xdr, networkPassphrase) {
  switch (walletId) {
    case WALLET_IDS.FREIGHTER:
      return freighterSign(xdr, networkPassphrase);
    case WALLET_IDS.XBULL:
      return xbullSign(xdr, networkPassphrase);
    case WALLET_IDS.LOBSTR:
      return lobstrSign(xdr, networkPassphrase);
    default:
      throw new Error(`Unknown wallet: ${walletId}`);
  }
}

/** Returns the last-used wallet ID from localStorage, or null. */
export function getSavedWalletId() {
  return localStorage.getItem(STORAGE_KEY);
}

/** Clears the persisted wallet selection. */
export function clearSavedWallet() {
  localStorage.removeItem(STORAGE_KEY);
}
