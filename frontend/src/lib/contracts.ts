/**
 * Configuration module for contract addresses.
 * Reads from environment variables (Vite-style) and validates their presence.
 */

const getEnvVar = (name: string): string => {
  const value = (import.meta.env as any)[name];
  if (!value) {
    throw new Error(`Environment variable ${name} is missing. Please check your .env file.`);
  }
  return value;
};

export const CONTRACT_IP_REGISTRY = getEnvVar("VITE_CONTRACT_IP_REGISTRY");
export const CONTRACT_ATOMIC_SWAP = getEnvVar("VITE_CONTRACT_ATOMIC_SWAP");
export const CONTRACT_ZK_VERIFIER = getEnvVar("VITE_CONTRACT_ZK_VERIFIER");

// Optional
export const CONTRACT_USDC = import.meta.env.VITE_CONTRACT_USDC || "";

export const STELLAR_NETWORK = import.meta.env.VITE_STELLAR_NETWORK || "testnet";
export const STELLAR_RPC_URL = import.meta.env.VITE_STELLAR_RPC_URL || "https://soroban-testnet.stellar.org";
export const IPFS_GATEWAY = import.meta.env.VITE_IPFS_GATEWAY || "https://gateway.pinata.cloud/ipfs";
