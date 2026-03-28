import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import {
  connectWallet,
  getSavedWalletId,
  clearSavedWallet,
  WALLET_IDS,
} from "../lib/walletKit";

const NETWORK_PASSPHRASE =
  import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? "Public Global Stellar Network ; September 2015"
    : "Test SDF Network ; September 2015";

const WalletContext = createContext(null);

/**
 * WalletProvider
 *
 * Provides wallet state to the entire app. On mount, attempts to
 * reconnect using the wallet ID persisted in localStorage.
 */
export function WalletProvider({ children }) {
  const [wallet, setWallet] = useState(null);   // { address, walletId, signTransaction }
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState(null);

  // Auto-reconnect on mount if a wallet was previously selected
  useEffect(() => {
    const savedId = getSavedWalletId();
    if (!savedId) return;

    setConnecting(true);
    connectWallet(savedId, NETWORK_PASSPHRASE)
      .then(setWallet)
      .catch(() => {
        // Silently clear stale persisted wallet — user will reconnect manually
        clearSavedWallet();
      })
      .finally(() => setConnecting(false));
  }, []);

  const connect = useCallback(async (walletId) => {
    setError(null);
    setConnecting(true);
    try {
      const w = await connectWallet(walletId, NETWORK_PASSPHRASE);
      setWallet(w);
    } catch (err) {
      setError(err.message || "Failed to connect wallet.");
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

/** Hook to consume wallet context. Must be used inside <WalletProvider>. */
export function useWallet() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error("useWallet must be used inside <WalletProvider>");
  return ctx;
}
