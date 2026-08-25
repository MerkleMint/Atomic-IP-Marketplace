/**
 * Per-network contract addresses, RPC endpoint, and passphrase, loaded from
 * .env via Vite.
 *
 * This is the single source of truth for "which network are we talking to" —
 * NetworkContext, contractClient, and the wallet-kit signing layer all derive
 * their configuration from getNetworkConfig() so they can't disagree about
 * contract IDs, RPC target, or passphrase.
 */
import * as StellarSdk from "@stellar/stellar-sdk";

export type Network = "testnet" | "mainnet";

export interface NetworkConfig {
  atomicSwap: string;
  ipRegistry: string;
  zkVerifier: string;
  usdc: string;
  rpcUrl: string;
  passphrase: string;
}

const NETWORK_CONFIGS: Record<Network, NetworkConfig> = {
  testnet: {
    atomicSwap: import.meta.env.VITE_CONTRACT_ATOMIC_SWAP ?? "",
    ipRegistry: import.meta.env.VITE_CONTRACT_IP_REGISTRY ?? "",
    zkVerifier: import.meta.env.VITE_CONTRACT_ZK_VERIFIER ?? "",
    usdc: import.meta.env.VITE_CONTRACT_USDC ?? "",
    rpcUrl: import.meta.env.VITE_STELLAR_RPC_URL ?? "",
    passphrase: StellarSdk.Networks.TESTNET,
  },
  mainnet: {
    atomicSwap: import.meta.env.VITE_MAINNET_CONTRACT_ATOMIC_SWAP ?? "",
    ipRegistry: import.meta.env.VITE_MAINNET_CONTRACT_IP_REGISTRY ?? "",
    zkVerifier: import.meta.env.VITE_MAINNET_CONTRACT_ZK_VERIFIER ?? "",
    usdc: import.meta.env.VITE_MAINNET_CONTRACT_USDC ?? "",
    rpcUrl: import.meta.env.VITE_MAINNET_STELLAR_RPC_URL ?? "",
    passphrase: StellarSdk.Networks.PUBLIC,
  },
};

/** Returns the full contract/RPC/passphrase config for the given network. */
export function getNetworkConfig(network: Network): NetworkConfig {
  return NETWORK_CONFIGS[network];
}

const required = {
  VITE_CONTRACT_ATOMIC_SWAP: NETWORK_CONFIGS.testnet.atomicSwap,
  VITE_CONTRACT_IP_REGISTRY: NETWORK_CONFIGS.testnet.ipRegistry,
  VITE_CONTRACT_ZK_VERIFIER: NETWORK_CONFIGS.testnet.zkVerifier,
  VITE_CONTRACT_USDC: NETWORK_CONFIGS.testnet.usdc,
  VITE_STELLAR_RPC_URL: NETWORK_CONFIGS.testnet.rpcUrl,
};

// Perform module-load validation for the default (testnet) network. Mainnet
// vars are optional at load time — an unconfigured mainnet contract surfaces
// a clear error the moment it's actually called, via the per-call checks in
// contractClient.ts.
Object.entries(required).forEach(([key, value]) => {
  if (!value || value.trim() === "") {
    throw new Error(
      `Frontend configuration error: ${key} is missing in .env file. ` +
        `Ensure all required VITE_* contract and network variables are set. ` +
        `Check .env.example for guidance.`
    );
  }
});
