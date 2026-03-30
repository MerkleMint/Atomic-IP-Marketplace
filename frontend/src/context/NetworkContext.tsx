import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import { reinitKit, WalletNetwork } from "../lib/walletKit";

export type Network = "testnet" | "mainnet";

interface NetworkContextValue {
  network: Network;
  setNetwork: (n: Network) => void;
  contractAddresses: {
    atomicSwap: string;
    ipRegistry: string;
    zkVerifier: string;
  };
}

const STORAGE_KEY = "selected_network";

const CONTRACTS: Record<Network, NetworkContextValue["contractAddresses"]> = {
  testnet: {
    atomicSwap: import.meta.env.VITE_CONTRACT_ATOMIC_SWAP ?? "",
    ipRegistry: import.meta.env.VITE_CONTRACT_IP_REGISTRY ?? "",
    zkVerifier: import.meta.env.VITE_CONTRACT_ZK_VERIFIER ?? "",
  },
  mainnet: {
    atomicSwap: import.meta.env.VITE_MAINNET_CONTRACT_ATOMIC_SWAP ?? "",
    ipRegistry: import.meta.env.VITE_MAINNET_CONTRACT_IP_REGISTRY ?? "",
    zkVerifier: import.meta.env.VITE_MAINNET_CONTRACT_ZK_VERIFIER ?? "",
  },
};

const NetworkContext = createContext<NetworkContextValue | null>(null);

export function NetworkProvider({ children }: { children: React.ReactNode }) {
  const [network, setNetworkState] = useState<Network>(() => {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved === "mainnet" ? "mainnet" : "testnet";
  });

  // Sync wallets-kit network on mount and change
  useEffect(() => {
    reinitKit(
      network === "mainnet" ? WalletNetwork.PUBLIC : WalletNetwork.TESTNET
    );
  }, [network]);

  const setNetwork = useCallback((n: Network) => {
    localStorage.setItem(STORAGE_KEY, n);
    setNetworkState(n);
  }, []);

  return (
    <NetworkContext.Provider
      value={{ network, setNetwork, contractAddresses: CONTRACTS[network as Network] }}
    >
      {children}
    </NetworkContext.Provider>
  );
}

export function useNetwork(): NetworkContextValue {
  const ctx = useContext(NetworkContext);
  if (!ctx) throw new Error("useNetwork must be used inside <NetworkProvider>");
  return ctx;
}
