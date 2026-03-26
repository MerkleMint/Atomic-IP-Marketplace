import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  type ReactNode,
} from "react";
import {
  connectWallet,
  getSavedWalletId,
  clearSavedWallet,
  WALLET_IDS,
  type ConnectedWallet,
  type WalletId,
} from "../lib/walletKit";

const NETWORK_PASSPHRASE =
  import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? "Public Global Stellar Network ; September 2015"
    : "Test SDF Network ; September 2015";

interface WalletContextValue {
  wallet: ConnectedWallet | null;
  connecting: boolean;
  error: string | null;
  connect: (walletId: string) => Promise<void>;
  disconnect: () => void;
  WALLET_IDS: typeof WALLET_IDS;
}

const WalletContext = createContext<WalletContextValue | null>(null);

export function WalletProvider({ children }: { children: ReactNode }) {
  const [wallet, setWallet] = useState<ConnectedWallet | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Auto-reconnect on mount if a wallet was previously selected
  useEffect(() => {
    const savedId = getSavedWalletId();
    if (!savedId) return;

    setConnecting(true);
    connectWallet(savedId, NETWORK_PASSPHRASE)
      .then(setWallet)
      .catch(() => clearSavedWallet())
      .finally(() => setConnecting(false));
  }, []);

  const connect = useCallback(async (walletId: string) => {
    setError(null);
    setConnecting(true);
    try {
      const w = await connectWallet(walletId, NETWORK_PASSPHRASE);
      setWallet(w);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to connect wallet.";
      setError(msg);
      throw err;
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    clearSavedWallet();
    setWallet(null);
    setError(null);
  }, []);

  return (
    <WalletContext.Provider value={{ wallet, connecting, error, connect, disconnect, WALLET_IDS }}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet(): WalletContextValue {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error("useWallet must be used inside <WalletProvider>");
  return ctx;
}
