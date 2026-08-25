import { describe, it, expect, beforeEach } from "vitest";
import { getCurrentNetwork, setCurrentNetwork } from "./network";

describe("network store", () => {
  beforeEach(() => {
    setCurrentNetwork("testnet");
  });

  it("updates the shared network immediately so other modules see it on their next read", () => {
    expect(getCurrentNetwork()).toBe("testnet");

    setCurrentNetwork("mainnet");
    expect(getCurrentNetwork()).toBe("mainnet");

    setCurrentNetwork("testnet");
    expect(getCurrentNetwork()).toBe("testnet");
  });
});
