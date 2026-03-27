import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import { connectWallet, getAvailableWallets, FREIGHTER_ID } from "../lib/walletKit";
import type { Wallet, ISupportedWallet } from "../lib/walletKit";

interface WalletContextValue {
  wallet: Wallet | null;
  connecting: boolean;
  error: string | null;
  availableWallets: ISupportedWallet[];
  connect: (walletId: string) => Promise<void>;
  disconnect: () => void;
}

const WalletContext = createContext<WalletContextValue | null>(null);

const STORAGE_KEY = "swk_wallet_id";

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const [wallet, setWallet] = useState<Wallet | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [availableWallets, setAvailableWallets] = useState<ISupportedWallet[]>([]);

  useEffect(() => {
    getAvailableWallets().then(setAvailableWallets).catch(() => {});
    const savedId = localStorage.getItem(STORAGE_KEY);
    if (!savedId) return;
    setConnecting(true);
    connectWallet(savedId)
      .then(setWallet)
      .catch(() => localStorage.removeItem(STORAGE_KEY))
      .finally(() => setConnecting(false));
  }, []);

  const connect = useCallback(async (walletId: string) => {
    setError(null);
    setConnecting(true);
    try {
      const w = await connectWallet(walletId);
      localStorage.setItem(STORAGE_KEY, walletId);
      setWallet(w);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to connect wallet.");
      throw err;
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    localStorage.removeItem(STORAGE_KEY);
    setWallet(null);
    setError(null);
  }, []);

  return (
    <WalletContext.Provider value={{ wallet, connecting, error, availableWallets, connect, disconnect }}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet(): WalletContextValue {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error("useWallet must be used inside <WalletProvider>");
  return ctx;
}

export { FREIGHTER_ID };
