/**
 * Proves the seller has an actual in-app path to collect payment: clicking
 * "Release Payment" calls release_to_seller via contractClient (no CLI step
 * required), it is gated by the escrow hold, and the buyer's "Confirm
 * Receipt & Release Early" action calls confirm_receipt. See issue #710.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ReleaseSwapActions } from "../src/components/ReleaseSwapActions";
import type { Swap } from "../src/hooks/useMySwaps";
import type { Wallet } from "../src/lib/walletKit";

const releaseToSeller = vi.fn();
const confirmReceipt = vi.fn();

vi.mock("../src/lib/contractClient", () => ({
  releaseToSeller: (...args: unknown[]) => releaseToSeller(...args),
  confirmReceipt: (...args: unknown[]) => confirmReceipt(...args),
}));

const SELLER_ADDRESS = "GSELLERAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const BUYER_ADDRESS = "GBUYERBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

const sellerWallet: Wallet = {
  address: SELLER_ADDRESS,
  walletId: "freighter",
  signTransaction: async (xdr: string) => xdr,
};
const buyerWallet: Wallet = { ...sellerWallet, address: BUYER_ADDRESS };

function makeSwap(overrides: Partial<Swap> = {}): Swap {
  return {
    id: 7,
    listing_id: 1,
    buyer: BUYER_ADDRESS,
    seller: SELLER_ADDRESS,
    usdc_amount: 1_000_0000000,
    usdc_token: "USDC_TOKEN",
    created_at: 0,
    expires_at: 0,
    status: "Completed",
    decryption_key: null,
    hold_until: null,
    buyer_confirmed: false,
    ...overrides,
  };
}

describe("ReleaseSwapActions", () => {
  beforeEach(() => {
    releaseToSeller.mockReset().mockResolvedValue(undefined);
    confirmReceipt.mockReset().mockResolvedValue(undefined);
  });

  it("lets the seller release payment once the hold has elapsed", async () => {
    const onSuccess = vi.fn();
    const swap = makeSwap({ hold_until: 1000, buyer_confirmed: false });
    render(
      <ReleaseSwapActions
        swap={swap}
        ledgerTimestamp={2000}
        wallet={sellerWallet}
        onSuccess={onSuccess}
      />
    );

    const button = screen.getByRole("button", { name: /release payment/i });
    expect(button).toBeEnabled();
    fireEvent.click(button);

    await waitFor(() =>
      expect(releaseToSeller).toHaveBeenCalledWith(7, sellerWallet)
    );
    await waitFor(() => expect(onSuccess).toHaveBeenCalled());
  });

  it("disables Release Payment while the escrow hold is still active", () => {
    const swap = makeSwap({ hold_until: 5000, buyer_confirmed: false });
    render(
      <ReleaseSwapActions
        swap={swap}
        ledgerTimestamp={1000}
        wallet={sellerWallet}
        onSuccess={vi.fn()}
      />
    );
    expect(screen.getByRole("button", { name: /release payment/i })).toBeDisabled();
  });

  it("lets the buyer confirm receipt to waive the remaining hold early", async () => {
    const onSuccess = vi.fn();
    const swap = makeSwap({ hold_until: 5000, buyer_confirmed: false });
    render(
      <ReleaseSwapActions
        swap={swap}
        ledgerTimestamp={1000}
        wallet={buyerWallet}
        onSuccess={onSuccess}
      />
    );

    const button = screen.getByRole("button", { name: /confirm receipt/i });
    fireEvent.click(button);

    await waitFor(() =>
      expect(confirmReceipt).toHaveBeenCalledWith(7, buyerWallet)
    );
    await waitFor(() => expect(onSuccess).toHaveBeenCalled());
  });

  it("does not offer a confirm-receipt action once the buyer has already confirmed", () => {
    const swap = makeSwap({ hold_until: 5000, buyer_confirmed: true });
    render(
      <ReleaseSwapActions
        swap={swap}
        ledgerTimestamp={1000}
        wallet={buyerWallet}
        onSuccess={vi.fn()}
      />
    );
    expect(
      screen.queryByRole("button", { name: /confirm receipt/i })
    ).not.toBeInTheDocument();
  });

  it("renders nothing before the swap has reached Completed", () => {
    const swap = makeSwap({ status: "Pending" });
    const { container } = render(
      <ReleaseSwapActions
        swap={swap}
        ledgerTimestamp={1000}
        wallet={sellerWallet}
        onSuccess={vi.fn()}
      />
    );
    expect(container).toBeEmptyDOMElement();
  });
});
