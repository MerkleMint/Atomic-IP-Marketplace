/**
 * Process-wide "which network is currently selected" store.
 *
 * NetworkContext owns the React-facing state and writes here on every
 * change; contractClient (and the wallet-kit init in walletKit.ts) read
 * from here so a network switch is reflected everywhere without threading
 * the network through every call site.
 */
import type { Network } from "./contracts";

export const NETWORK_STORAGE_KEY = "selected_network";

function readStoredNetwork(): Network {
  try {
    return localStorage.getItem(NETWORK_STORAGE_KEY) === "mainnet"
      ? "mainnet"
      : "testnet";
  } catch {
    return "testnet";
  }
}

let currentNetwork: Network = readStoredNetwork();

export function getCurrentNetwork(): Network {
  return currentNetwork;
}

export function setCurrentNetwork(network: Network): void {
  currentNetwork = network;
}
