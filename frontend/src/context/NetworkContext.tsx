import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import { reinitKit, WalletNetwork } from "../lib/walletKit";
import { getNetworkConfig } from "../lib/contracts";
import type { Network } from "../lib/contracts";
import { getCurrentNetwork, setCurrentNetwork, NETWORK_STORAGE_KEY } from "../lib/network";

export type { Network };

interface NetworkContextValue {
  network: Network;
  setNetwork: (n: Network) => void;
  contractAddresses: {
    atomicSwap: string;
    ipRegistry: string;
    zkVerifier: string;
  };
}

const NetworkContext = createContext<NetworkContextValue | null>(null);

export function NetworkProvider({ children }: { children: React.ReactNode }) {
  const [network, setNetworkState] = useState<Network>(getCurrentNetwork);

  // Sync wallet-kit's signing network on mount and change. Both this and the
  // transaction-building network in contractClient derive from the same
  // getNetworkConfig(network) source, so they can't disagree.
  useEffect(() => {
    reinitKit(
      network === "mainnet" ? WalletNetwork.PUBLIC : WalletNetwork.TESTNET
    );
  }, [network]);

  const setNetwork = useCallback((n: Network) => {
    localStorage.setItem(NETWORK_STORAGE_KEY, n);
    setCurrentNetwork(n);
    setNetworkState(n);
  }, []);

  const config = getNetworkConfig(network);

  return (
    <NetworkContext.Provider
      value={{
        network,
        setNetwork,
        contractAddresses: {
          atomicSwap: config.atomicSwap,
          ipRegistry: config.ipRegistry,
          zkVerifier: config.zkVerifier,
        },
      }}
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
