import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { Network } from "../lib/contracts";

vi.mock("../lib/walletKit", () => ({
  reinitKit: vi.fn(),
  WalletNetwork: {
    PUBLIC: "Public Global Stellar Network ; September 2015",
    TESTNET: "Test SDF Network ; September 2015",
  },
}));

import { reinitKit, WalletNetwork } from "../lib/walletKit";
import { NetworkProvider, useNetwork } from "./NetworkContext";
import { getCurrentNetwork } from "../lib/network";
import { getNetworkConfig } from "../lib/contracts";

type NetworkApi = ReturnType<typeof useNetwork>;

function Probe({ onReady }: { onReady: (api: NetworkApi) => void }) {
  const api = useNetwork();
  onReady(api);
  return null;
}

function renderNetworkProbe() {
  let api!: NetworkApi;
  render(
    <NetworkProvider>
      <Probe onReady={(a) => (api = a)} />
    </NetworkProvider>
  );
  return {
    getApi: () => api,
  };
}

describe("NetworkContext reactivity", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(reinitKit).mockClear();
  });

  it("propagates setNetwork('mainnet') to the shared network store contractClient reads from", () => {
    const { getApi } = renderNetworkProbe();
    expect(getApi().network).toBe("testnet");
    expect(getCurrentNetwork()).toBe("testnet");

    act(() => getApi().setNetwork("mainnet"));

    expect(getApi().network).toBe("mainnet");
    expect(getCurrentNetwork()).toBe("mainnet");
  });

  it("keeps reinitKit's wallet-signing network and the tx-building passphrase in lockstep for every selection", () => {
    const { getApi } = renderNetworkProbe();

    const selections: Network[] = ["mainnet", "testnet", "mainnet"];
    for (const network of selections) {
      act(() => getApi().setNetwork(network));

      const calls = vi.mocked(reinitKit).mock.calls;
      const walletNetworkArg = calls[calls.length - 1]?.[0];
      const expectedWalletNetwork =
        network === "mainnet" ? WalletNetwork.PUBLIC : WalletNetwork.TESTNET;
      expect(walletNetworkArg).toBe(expectedWalletNetwork);

      // Same value contractClient uses to build/verify transaction envelopes.
      const txPassphrase = getNetworkConfig(network).passphrase;
      expect(walletNetworkArg as string).toBe(txPassphrase);
    }
  });
});
