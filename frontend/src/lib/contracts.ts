/**
 * Contract addresses and network settings loaded from .env via Vite.
 * This module exports these values as typed constants and performs
 * a validation check to ensure the application does not run with
 * missing configuration.
 *
 * Each VITE_* variable should match your deployment settings for
 * the Stellar network.
 */

export const CONTRACT_ATOMIC_SWAP: string = import.meta.env.VITE_CONTRACT_ATOMIC_SWAP;
export const CONTRACT_IP_REGISTRY: string = import.meta.env.VITE_CONTRACT_IP_REGISTRY;
export const CONTRACT_ZK_VERIFIER: string = import.meta.env.VITE_CONTRACT_ZK_VERIFIER;
export const CONTRACT_USDC: string = import.meta.env.VITE_CONTRACT_USDC;
export const STELLAR_NETWORK: string = import.meta.env.VITE_STELLAR_NETWORK;
export const STELLAR_RPC_URL: string = import.meta.env.VITE_STELLAR_RPC_URL;

const required = {
  VITE_CONTRACT_ATOMIC_SWAP: CONTRACT_ATOMIC_SWAP,
  VITE_CONTRACT_IP_REGISTRY: CONTRACT_IP_REGISTRY,
  VITE_CONTRACT_ZK_VERIFIER: CONTRACT_ZK_VERIFIER,
  VITE_CONTRACT_USDC: CONTRACT_USDC,
  VITE_STELLAR_NETWORK: STELLAR_NETWORK,
  VITE_STELLAR_RPC_URL: STELLAR_RPC_URL,
};

// Perform module-load validation
Object.entries(required).forEach(([key, value]) => {
  if (!value || value.trim() === "") {
    throw new Error(
      `Frontend configuration error: ${key} is missing in .env file. ` +
        `Ensure all six VITE_* contract and network variables are set. ` +
        `Check .env.example for guidance.`
    );
  }
});
