/**
 * walletKit.ts
 *
 * Wallet abstraction built on @creit-tech/stellar-wallets-kit.
 * Supports Freighter, xBull, and Lobstr via a unified StellarWalletsKit instance.
 *
 * Exported wallet object shape (consumed by WalletContext and contractClient):
 *   { address: string, walletId: string, signTransaction: (xdr: string) => Promise<string> }
 */

import {
  StellarWalletsKit,
  WalletNetwork,
  FREIGHTER_ID,
  XBULL_ID,
  FreighterModule,
  xBullModule,
  // Lobstr is available as a community module; fall back to window.lobstr if absent
} from "@creit-tech/stellar-wallets-kit";

// ─── Constants ────────────────────────────────────────────────────────────────

export const WALLET_IDS = {
  FREIGHTER: FREIGHTER_ID,
  XBULL: XBULL_ID,
  LOBSTR: "lobstr",
} as const;

export type WalletId = (typeof WALLET_IDS)[keyof typeof WALLET_IDS];

const STORAGE_KEY = "swk_wallet_id";

// ─── Kit singleton ────────────────────────────────────────────────────────────

function buildKit(network: WalletNetwork): StellarWalletsKit {
  return new StellarWalletsKit({
    network,
    selectedWalletId: FREIGHTER_ID,
    modules: [new FreighterModule(), new xBullModule()],
  });
}

// ─── Public API ───────────────────────────────────────────────────────────────

export interface ConnectedWallet {
  address: string;
  walletId: string;
  signTransaction: (xdr: string) => Promise<string>;
}

/**
 * Returns true if the given wallet extension appears to be installed.
 */
export function isWalletAvailable(walletId: string): boolean {
  if (typeof window === "undefined") return false;
  switch (walletId) {
    case WALLET_IDS.FREIGHTER:
      return !!(window as any).freighter;
    case WALLET_IDS.XBULL:
      return !!(window as any).xBullSDK || !!(window as any).xbull;
    case WALLET_IDS.LOBSTR:
      return !!(window as any).lobstr;
    default:
      return false;
  }
}

/**
 * Connect to a wallet by ID using stellar-wallets-kit.
 * Persists the selection to localStorage for auto-reconnect.
 */
export async function connectWallet(
  walletId: string,
  networkPassphrase: string
): Promise<ConnectedWallet> {
  const network =
    networkPassphrase.includes("Public Global")
      ? WalletNetwork.PUBLIC
      : WalletNetwork.TESTNET;

  // Lobstr injects window.lobstr — handle it outside the kit
  if (walletId === WALLET_IDS.LOBSTR) {
    return connectLobstr(networkPassphrase);
  }

  const kit = buildKit(network);
  kit.setWallet(walletId);

  const { address } = await kit.getAddress();
  localStorage.setItem(STORAGE_KEY, walletId);

  return {
    address,
    walletId,
    signTransaction: async (xdr: string) => {
      const { signedTxXdr } = await kit.signTransaction(xdr, {
        networkPassphrase,
      });
      return signedTxXdr;
    },
  };
}

// ─── Lobstr fallback (window.lobstr) ─────────────────────────────────────────

async function connectLobstr(networkPassphrase: string): Promise<ConnectedWallet> {
  const lobstr = (window as any).lobstr;
  if (!lobstr) throw new Error("Lobstr Signer Extension is not installed.");

  const address: string = await lobstr.getPublicKey();
  localStorage.setItem(STORAGE_KEY, WALLET_IDS.LOBSTR);

  return {
    address,
    walletId: WALLET_IDS.LOBSTR,
    signTransaction: async (xdr: string) => {
      const result = await lobstr.signTransaction(xdr, { networkPassphrase });
      return typeof result === "string" ? result : (result.signedXdr ?? result.signedTxXdr);
    },
  };
}

// ─── Persistence helpers ──────────────────────────────────────────────────────

export function getSavedWalletId(): string | null {
  return localStorage.getItem(STORAGE_KEY);
}

export function clearSavedWallet(): void {
  localStorage.removeItem(STORAGE_KEY);
}
