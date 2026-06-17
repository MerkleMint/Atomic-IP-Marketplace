/**
 * Mocked contract responses for integration testing
 * Simulates realistic contract behavior without requiring actual RPC calls
 */

import { Swap } from "../../../hooks/useMySwaps";

export const createMockSwap = (overrides?: Partial<Swap>): Swap => ({
  id: 1,
  listing_id: 100,
  buyer: "GBUQWP3BOUZX34ULNQG23RQ6F4OFSXHYXUCG7JBSXQB5E74Z5BMFBF2P",
  seller: "GBXYZ123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ",
  usdc_amount: 1000000000, // 100 USDC (7 decimals)
  usdc_token: "USDC_CONTRACT_ID",
  created_at: Math.floor(Date.now() / 1000) - 3600,
  expires_at: Math.floor(Date.now() / 1000) + 3600,
  status: "Pending",
  decryption_key: null,
  ...overrides,
});

/**
 * Simulate contract polling with realistic delays
 * Returns a promise that resolves with updated swap data
 */
export const mockGetSwapWithDelay = (
  swapId: number,
  status: "Pending" | "Completed" | "Cancelled" = "Pending",
  delayMs = 100
): Promise<Swap> => {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve(
        createMockSwap({
          id: swapId,
          status,
        })
      );
    }, delayMs);
  });
};

/**
 * Simulate network error for testing error handling
 */
export const mockNetworkError = (
  message = "Failed to fetch swap data"
): Promise<never> => {
  return Promise.reject(new Error(message));
};

/**
 * Simulate progressive status updates
 * Useful for testing real-time update flows
 */
export const createStatusProgression = (swapId: number) => {
  const statuses: Array<"Pending" | "Completed" | "Cancelled"> = [
    "Pending",
    "Pending",
    "Pending",
    "Completed",
  ];
  let index = 0;

  return {
    getNext: () => {
      const status = statuses[Math.min(index, statuses.length - 1)];
      index++;
      return createMockSwap({ id: swapId, status });
    },
    reset: () => {
      index = 0;
    },
  };
};

/**
 * Simulate confirmation progress
 */
export const createConfirmationProgress = (targetConfirmations = 3) => {
  let confirmations = 0;

  return {
    getNext: () => {
      confirmations = Math.min(confirmations + 1, targetConfirmations);
      return confirmations;
    },
    reset: () => {
      confirmations = 0;
    },
    current: () => confirmations,
  };
};

/**
 * Mock transaction history with various statuses
 */
export const mockTransactionHistory = (walletAddress: string): Swap[] => {
  const baseTime = Math.floor(Date.now() / 1000);
  return [
    createMockSwap({
      id: 1,
      buyer: walletAddress,
      status: "Pending",
      created_at: baseTime - 1800,
      expires_at: baseTime + 1800,
    }),
    createMockSwap({
      id: 2,
      buyer: walletAddress,
      status: "Completed",
      created_at: baseTime - 7200,
      expires_at: baseTime - 3600,
      decryption_key: "abc123def456",
    }),
    createMockSwap({
      id: 3,
      seller: walletAddress,
      status: "Completed",
      created_at: baseTime - 14400,
      expires_at: baseTime - 10800,
    }),
    createMockSwap({
      id: 4,
      buyer: walletAddress,
      status: "Cancelled",
      created_at: baseTime - 21600,
      expires_at: baseTime - 18000,
    }),
  ];
};
