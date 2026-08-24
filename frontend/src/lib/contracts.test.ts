import { describe, it, expect } from "vitest";
import * as StellarSdk from "@stellar/stellar-sdk";
import { getNetworkConfig } from "./contracts";

describe("getNetworkConfig", () => {
  it("resolves testnet contract IDs, RPC URL, and passphrase from the testnet env vars", () => {
    const cfg = getNetworkConfig("testnet");
    expect(cfg.atomicSwap).toBe(import.meta.env.VITE_CONTRACT_ATOMIC_SWAP);
    expect(cfg.ipRegistry).toBe(import.meta.env.VITE_CONTRACT_IP_REGISTRY);
    expect(cfg.zkVerifier).toBe(import.meta.env.VITE_CONTRACT_ZK_VERIFIER);
    expect(cfg.usdc).toBe(import.meta.env.VITE_CONTRACT_USDC);
    expect(cfg.rpcUrl).toBe(import.meta.env.VITE_STELLAR_RPC_URL);
    expect(cfg.passphrase).toBe(StellarSdk.Networks.TESTNET);
  });

  it("resolves mainnet contract IDs, RPC URL, and passphrase distinct from testnet", () => {
    const testnet = getNetworkConfig("testnet");
    const mainnet = getNetworkConfig("mainnet");

    expect(mainnet.atomicSwap).toBe(import.meta.env.VITE_MAINNET_CONTRACT_ATOMIC_SWAP);
    expect(mainnet.ipRegistry).toBe(import.meta.env.VITE_MAINNET_CONTRACT_IP_REGISTRY);
    expect(mainnet.zkVerifier).toBe(import.meta.env.VITE_MAINNET_CONTRACT_ZK_VERIFIER);
    expect(mainnet.usdc).toBe(import.meta.env.VITE_MAINNET_CONTRACT_USDC);
    expect(mainnet.rpcUrl).toBe(import.meta.env.VITE_MAINNET_STELLAR_RPC_URL);
    expect(mainnet.passphrase).toBe(StellarSdk.Networks.PUBLIC);

    expect(mainnet.atomicSwap).not.toBe(testnet.atomicSwap);
    expect(mainnet.rpcUrl).not.toBe(testnet.rpcUrl);
    expect(mainnet.passphrase).not.toBe(testnet.passphrase);
  });
});
