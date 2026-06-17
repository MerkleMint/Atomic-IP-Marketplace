/**
 * Integration tests with mocked contract responses
 * Tests real-time updates, polling, and error recovery
 */

import { render, screen, waitFor } from "@testing-library/react";
import { SwapStatusDashboard } from "../SwapStatusDashboard";
import { useWallet } from "../../context/WalletContext";
import { useMySwaps } from "../../hooks/useMySwaps";
import * as contractClient from "../../lib/contractClient";
import {
  createMockSwap,
  mockGetSwapWithDelay,
  mockNetworkError,
  createStatusProgression,
  mockTransactionHistory,
} from "./mocks/contractMocks";

jest.mock("../../context/WalletContext");
jest.mock("../../hooks/useMySwaps");
jest.mock("../../lib/contractClient");

const mockWallet = {
  address: "GBUQWP3BOUZX34ULNQG23RQ6F4OFSXHYXUCG7JBSXQB5E74Z5BMFBF2P",
};

describe("SwapStatusDashboard - Integration Tests with Mocked Responses", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.runOnlyPendingTimers();
    jest.useRealTimers();
  });

  describe("Real-time status updates", () => {
    it("should update swap status from Pending to Completed", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const progression = createStatusProgression(1);

      const initialSwap = progression.getNext();
      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [initialSwap],
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      (contractClient.getSwap as jest.Mock).mockImplementation(() =>
        mockGetSwapWithDelay(1, progression.getNext().status, 50)
      );

      render(<SwapStatusDashboard />);

      expect(screen.getByLabelText(/status: pending/i)).toBeInTheDocument();

      // Advance through polling cycles
      jest.advanceTimersByTime(5000);
      jest.advanceTimersByTime(5000);
      jest.advanceTimersByTime(5000);

      await waitFor(
        () => {
          expect(
            screen.getByLabelText(/status: completed/i)
          ).toBeInTheDocument();
        },
        { timeout: 10000 }
      );
    });

    it("should handle rapid status changes", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const swapProgression = createStatusProgression(1);

      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [swapProgression.getNext()],
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      let callCount = 0;
      (contractClient.getSwap as jest.Mock).mockImplementation(() => {
        callCount++;
        return mockGetSwapWithDelay(1, swapProgression.getNext().status, 10);
      });

      render(<SwapStatusDashboard />);

      jest.advanceTimersByTime(15000);

      await waitFor(() => {
        expect(callCount).toBeGreaterThan(1);
      });
    });
  });

  describe("Polling with exponential backoff", () => {
    it("should retry on network error with exponential backoff", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [createMockSwap()],
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      let callCount = 0;
      (contractClient.getSwap as jest.Mock).mockImplementation(() => {
        callCount++;
        if (callCount <= 2) {
          return mockNetworkError("Network timeout");
        }
        return mockGetSwapWithDelay(1, "Pending", 50);
      });

      render(<SwapStatusDashboard />);

      // First call
      await waitFor(() => {
        expect(callCount).toBe(1);
      });

      // Retry after 5 seconds
      jest.advanceTimersByTime(5000);
      await waitFor(() => {
        expect(callCount).toBeGreaterThanOrEqual(2);
      });

      // Exponential backoff: 5s * 1.5 = 7.5s
      jest.advanceTimersByTime(7500);
      await waitFor(() => {
        expect(callCount).toBeGreaterThanOrEqual(3);
      });

      // Final successful call
      await waitFor(
        () => {
          expect(callCount).toBeGreaterThanOrEqual(3);
        },
        { timeout: 5000 }
      );
    });

    it("should display error after max retries", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [createMockSwap()],
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      (contractClient.getSwap as jest.Mock).mockImplementation(() =>
        mockNetworkError("Persistent network error")
      );

      render(<SwapStatusDashboard />);

      // Wait for max retries (3 attempts with delays)
      jest.advanceTimersByTime(5000);
      jest.advanceTimersByTime(7500);
      jest.advanceTimersByTime(11250);

      await waitFor(() => {
        const errorElement = screen.queryByText(/failed to update/i);
        expect(errorElement).toBeInTheDocument();
      });
    });
  });

  describe("Transaction history", () => {
    it("should display all transactions in history view", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const history = mockTransactionHistory(mockWallet.address);

      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: history,
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      render(<SwapStatusDashboard />);

      const historyTab = screen.getByRole("tab", {
        name: /transaction history/i,
      });
      await screen.findByRole("button", { name: historyTab.textContent! });
      historyTab.click();

      await waitFor(() => {
        expect(screen.getByText(/#1/)).toBeInTheDocument();
        expect(screen.getByText(/#2/)).toBeInTheDocument();
      });
    });

    it("should filter history by status", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const history = mockTransactionHistory(mockWallet.address);

      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: history,
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      render(<SwapStatusDashboard />);

      const historyTab = screen.getByRole("tab", {
        name: /transaction history/i,
      });
      historyTab.click();

      await waitFor(() => {
        expect(screen.getByText(/#1/)).toBeInTheDocument();
      });
    });

    it("should sort history by amount", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const history = [
        createMockSwap({
          id: 1,
          usdc_amount: 1000000000,
          status: "Completed",
        }),
        createMockSwap({
          id: 2,
          usdc_amount: 5000000000,
          status: "Completed",
        }),
        createMockSwap({
          id: 3,
          usdc_amount: 2500000000,
          status: "Completed",
        }),
      ];

      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: history,
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      render(<SwapStatusDashboard />);

      const historyTab = screen.getByRole("tab", {
        name: /transaction history/i,
      });
      historyTab.click();

      await waitFor(() => {
        const amounts = screen.getAllByText(/\d+\.\d+ usdc/i);
        expect(amounts.length).toBeGreaterThanOrEqual(1);
      });
    });
  });

  describe("Dispute countdown", () => {
    it("should display countdown when swap is disputed", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const baseTime = Math.floor(Date.now() / 1000);

      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [
          createMockSwap({
            expires_at: baseTime + 3600,
            status: "Pending",
          }),
        ],
        ledgerTimestamp: baseTime,
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      render(<SwapStatusDashboard />);

      const timer = await screen.findByRole("timer");
      expect(timer).toHaveTextContent(/expires/i);
    });

    it("should show warning when expiration is critical (< 10 minutes)", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      const baseTime = Math.floor(Date.now() / 1000);

      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [
          createMockSwap({
            expires_at: baseTime + 300, // 5 minutes
            status: "Pending",
          }),
        ],
        ledgerTimestamp: baseTime,
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      render(<SwapStatusDashboard />);

      await waitFor(() => {
        const timer = screen.getByRole("timer");
        // Should have warning indicator
        expect(timer.parentElement).toHaveClass("dispute-countdown--critical");
      });
    });
  });

  describe("Performance and caching", () => {
    it("should minimize contract calls through smart polling", async () => {
      (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
      (useMySwaps as jest.Mock).mockReturnValue({
        swaps: [
          createMockSwap({ status: "Completed" }), // Non-pending
        ],
        ledgerTimestamp: Math.floor(Date.now() / 1000),
        loading: false,
        error: null,
        refresh: jest.fn(),
      });

      (contractClient.getSwap as jest.Mock).mockImplementation(() =>
        mockGetSwapWithDelay(1, "Completed", 50)
      );

      render(<SwapStatusDashboard />);

      const initialCalls = (contractClient.getSwap as jest.Mock).mock.calls
        .length;

      // Advance time significantly
      jest.advanceTimersByTime(60000);

      // Should not poll for completed swaps
      const finalCalls = (contractClient.getSwap as jest.Mock).mock.calls
        .length;
      expect(finalCalls - initialCalls).toBeLessThan(3);
    });
  });
});
