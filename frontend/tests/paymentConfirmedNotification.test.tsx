/**
 * Proves "Payment Received" is not sent when a swap merely reaches
 * "Completed" (funds are still in escrow at that point) and only fires once
 * the swap actually reaches "ResolvedSeller" (release_to_seller succeeded,
 * or the seller was awarded funds via dispute resolution). See issue #710.
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import {
  NotificationProvider,
  useNotifications,
  useSwapNotifications,
} from "../src/context/NotificationContext";

const SELLER_ADDRESS = "GSELLERAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const BUYER_ADDRESS = "GBUYERBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

vi.mock("../src/context/WalletContext", () => ({
  useWallet: () => ({
    wallet: { address: SELLER_ADDRESS, walletId: "freighter", signTransaction: async (x: string) => x },
  }),
}));

function makeSwap(status: string) {
  return {
    id: 42,
    listing_id: 1,
    buyer: BUYER_ADDRESS,
    seller: SELLER_ADDRESS,
    status,
    usdc_amount: 500_0000000,
    expires_at: Math.floor(Date.now() / 1000) + 3600,
  };
}

function Harness({ status }: { status: string }) {
  useSwapNotifications([makeSwap(status)], SELLER_ADDRESS);
  const { notifications } = useNotifications();
  return (
    <ul>
      {notifications.map((n) => (
        <li key={n.id}>{n.title}</li>
      ))}
    </ul>
  );
}

describe("payment_confirmed notification timing", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("only fires once the swap reaches ResolvedSeller, not when it reaches Completed", async () => {
    const { rerender } = render(
      <NotificationProvider>
        <Harness status="Pending" />
      </NotificationProvider>
    );

    rerender(
      <NotificationProvider>
        <Harness status="Completed" />
      </NotificationProvider>
    );

    // Funds are still escrowed here — no "Payment Received" yet.
    await waitFor(() => {
      expect(screen.queryByText("Payment Received")).not.toBeInTheDocument();
    });

    rerender(
      <NotificationProvider>
        <Harness status="ResolvedSeller" />
      </NotificationProvider>
    );

    // release_to_seller has now actually moved the funds.
    await waitFor(() => {
      expect(screen.getByText("Payment Received")).toBeInTheDocument();
    });
  });
});
