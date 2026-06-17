/**
 * Integration tests for SwapStatusDashboard component
 * Tests polling logic, real-time updates, and error handling
 */

import { render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { SwapStatusDashboard } from "../SwapStatusDashboard";
import { useWallet } from "../../context/WalletContext";
import { useMySwaps } from "../../hooks/useMySwaps";
import * as contractClient from "../../lib/contractClient";

// Mock dependencies
jest.mock("../../context/WalletContext");
jest.mock("../../hooks/useMySwaps");
jest.mock("../../lib/contractClient");

const mockWallet = {
  address: "GBUQWP3BOUZX34ULNQG23RQ6F4OFSXHYXUCG7JBSXQB5E74Z5BMFBF2P",
};

const mockSwap = {
  id: 1,
  listing_id: 100,
  buyer: mockWallet.address,
  seller: "GBXYZ...",
  usdc_amount: 1000000000,
  usdc_token: "USDC_TOKEN",
  created_at: Math.floor(Date.now() / 1000) - 3600,
  expires_at: Math.floor(Date.now() / 1000) + 3600,
  status: "Pending",
  decryption_key: null,
};

describe("SwapStatusDashboard", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.runOnlyPendingTimers();
    jest.useRealTimers();
  });

  it("should display wallet connection prompt when not connected", () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: null });

    render(<SwapStatusDashboard />);

    expect(
      screen.getByText(/connect your wallet to view swap status/i)
    ).toBeInTheDocument();
  });

  it("should display loading state while fetching swaps", () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: true,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    expect(screen.getByLabelText(/loading/i)).toBeInTheDocument();
  });

  it("should display active swaps in card grid", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    await waitFor(() => {
      expect(screen.getByText(/swap #1/i)).toBeInTheDocument();
      expect(screen.getByText(/100\.00 usdc/i)).toBeInTheDocument();
    });
  });

  it("should display status badge with pending state", () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    expect(screen.getByLabelText(/status: pending/i)).toBeInTheDocument();
  });

  it("should display dispute countdown timer", () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    const expiresAt = Math.floor(Date.now() / 1000) + 3600;
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [{ ...mockSwap, expires_at: expiresAt }],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    expect(screen.getByRole("timer")).toBeInTheDocument();
  });

  it("should poll contract state with exponential backoff on error", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    const mockGetSwap = jest.fn();
    (contractClient.getSwap as jest.Mock) = mockGetSwap;

    // Simulate first call error
    mockGetSwap.mockRejectedValueOnce(new Error("Network error"));

    render(<SwapStatusDashboard />);

    await waitFor(() => {
      expect(mockGetSwap).toHaveBeenCalledWith(1);
    });

    // First retry after 5 seconds
    jest.advanceTimersByTime(5000);
    await waitFor(() => {
      expect(mockGetSwap).toHaveBeenCalledTimes(2);
    });

    // Second retry after 7.5 seconds (exponential backoff)
    jest.advanceTimersByTime(7500);
    await waitFor(() => {
      expect(mockGetSwap).toHaveBeenCalledTimes(3);
    });
  });

  it("should update swap status in real-time", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    const initialSwap = { ...mockSwap, status: "Pending" };
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [initialSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    const updatedSwap = { ...mockSwap, status: "Completed" };
    (contractClient.getSwap as jest.Mock).mockResolvedValue(updatedSwap);

    const { rerender } = render(<SwapStatusDashboard />);

    // Simulate polling update
    jest.advanceTimersByTime(5000);

    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [updatedSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    rerender(<SwapStatusDashboard />);

    await waitFor(() => {
      expect(screen.getByLabelText(/status: completed/i)).toBeInTheDocument();
    });
  });

  it("should handle connection errors gracefully", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: "Failed to connect to RPC",
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText(/failed to connect to rpc/i)).toBeInTheDocument();
  });

  it("should pause polling when page is hidden", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    (contractClient.getSwap as jest.Mock).mockResolvedValue(mockSwap);

    render(<SwapStatusDashboard />);

    const initialCallCount = (contractClient.getSwap as jest.Mock).mock
      .calls.length;

    // Simulate tab hidden
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));

    jest.advanceTimersByTime(10000);

    // Calls should not increase significantly while hidden
    expect((contractClient.getSwap as jest.Mock).mock.calls.length).toBeLessThan(
      initialCallCount + 3
    );
  });

  it("should switch between active and history tabs", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    const pendingSwap = { ...mockSwap, status: "Pending" };
    const completedSwap = { ...mockSwap, id: 2, status: "Completed" };

    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [pendingSwap, completedSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    // Should show active swaps initially
    expect(screen.getByText(/swap #1/i)).toBeInTheDocument();
    expect(screen.queryByText(/swap #2/i)).not.toBeInTheDocument();

    // Switch to history
    const historyTab = screen.getByRole("tab", { name: /transaction history/i });
    await userEvent.click(historyTab);

    await waitFor(() => {
      expect(screen.getByText(/swap #2/i)).toBeInTheDocument();
    });
  });

  it("should filter and sort transaction history", async () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [
        { ...mockSwap, id: 1, usdc_amount: 1000000000, status: "Pending" },
        { ...mockSwap, id: 2, usdc_amount: 5000000000, status: "Completed" },
      ],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    // Switch to history tab
    const historyTab = screen.getByRole("tab", { name: /transaction history/i });
    await userEvent.click(historyTab);

    await waitFor(() => {
      // Check filter functionality
      const filters = screen.getAllByRole("radio");
      await userEvent.click(filters[1]); // Click "Active" filter
    });
  });

  it("should be keyboard navigable", async () => {
    const user = userEvent.setup();
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    render(<SwapStatusDashboard />);

    const refreshBtn = screen.getByRole("button", { name: /refresh/i });
    await user.tab();
    expect(refreshBtn).toHaveFocus();

    const activeTab = screen.getByRole("tab", { name: /active swaps/i });
    await user.tab();
    expect(activeTab).toHaveFocus();
  });

  it("should be accessible with screen readers", () => {
    (useWallet as jest.Mock).mockReturnValue({ wallet: mockWallet });
    (useMySwaps as jest.Mock).mockReturnValue({
      swaps: [mockSwap],
      ledgerTimestamp: Math.floor(Date.now() / 1000),
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    const { container } = render(<SwapStatusDashboard />);

    // Check semantic HTML
    expect(container.querySelector("section[aria-label]")).toBeInTheDocument();
    expect(screen.getByRole("tablist")).toBeInTheDocument();
    expect(screen.getAllByRole("tab")).toHaveLength(2);
  });
});
