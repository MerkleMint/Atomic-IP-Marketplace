import { describe, it, expect, beforeEach, vi } from "vitest";

const serverConstructions: string[] = [];
const contractConstructions: string[] = [];

vi.mock("@stellar/stellar-sdk", () => {
  class ServerMock {
    constructor(rpcUrl: string) {
      serverConstructions.push(rpcUrl);
    }
    async simulateTransaction() {
      return { result: { retval: {} } };
    }
  }
  class ContractMock {
    constructor(contractId: string) {
      contractConstructions.push(contractId);
    }
    call() {
      return {};
    }
  }
  class TransactionBuilderMock {
    constructor(_account: unknown, _opts: unknown) {}
    addOperation() {
      return this;
    }
    setTimeout() {
      return this;
    }
    build() {
      return {};
    }
  }
  class KeypairMock {
    static random() {
      return { publicKey: () => "GRANDOMSOURCEACCOUNT" };
    }
  }
  class AccountMock {
    constructor(_id: string, _seq: string) {}
  }

  return {
    SorobanRpc: {
      Server: ServerMock,
      Api: { isSimulationError: () => false },
    },
    Contract: ContractMock,
    TransactionBuilder: TransactionBuilderMock,
    Keypair: KeypairMock,
    Account: AccountMock,
    Address: class {
      constructor(_id: string) {}
    },
    nativeToScVal: (v: unknown) => v,
    scValToNative: () => 0,
    xdr: {
      ScVal: {
        scvVec: () => ({}),
        scvBytes: () => ({}),
        scvBool: () => ({}),
        scvSymbol: () => ({}),
      },
      ScMapEntry: class {
        constructor(_entry: unknown) {}
      },
    },
    BASE_FEE: "100",
    Networks: {
      PUBLIC: "Public Global Stellar Network ; September 2015",
      TESTNET: "Test SDF Network ; September 2015",
    },
  };
});

import { setCurrentNetwork } from "./network";
import { getNetworkConfig } from "./contracts";
import { getListingCount } from "./contractClient";

describe("contractClient network targeting", () => {
  beforeEach(() => {
    serverConstructions.length = 0;
    contractConstructions.length = 0;
    setCurrentNetwork("testnet");
  });

  it("reads from the testnet RPC endpoint and contract ID by default", async () => {
    await getListingCount();

    const testnetCfg = getNetworkConfig("testnet");
    const lastServer = serverConstructions[serverConstructions.length - 1];
    const lastContract = contractConstructions[contractConstructions.length - 1];
    expect(lastServer).toBe(testnetCfg.rpcUrl);
    expect(lastContract).toBe(testnetCfg.ipRegistry);
  });

  it("targets the mainnet RPC endpoint and contract ID after setNetwork('mainnet')", async () => {
    setCurrentNetwork("mainnet");
    await getListingCount();

    const mainnetCfg = getNetworkConfig("mainnet");
    const testnetCfg = getNetworkConfig("testnet");
    const lastServer = serverConstructions[serverConstructions.length - 1];
    const lastContract = contractConstructions[contractConstructions.length - 1];
    expect(lastServer).toBe(mainnetCfg.rpcUrl);
    expect(lastContract).toBe(mainnetCfg.ipRegistry);
    expect(lastServer).not.toBe(testnetCfg.rpcUrl);
    expect(lastContract).not.toBe(testnetCfg.ipRegistry);
  });
});
