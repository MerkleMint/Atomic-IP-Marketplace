import * as StellarSdk from "@stellar/stellar-sdk";
import { USDC_DECIMALS } from "./types";
import { getNetworkConfig } from "./contracts";
import { getCurrentNetwork } from "./network";

/** Resolve contract IDs, RPC URL, and passphrase for the currently selected network. */
function currentConfig() {
  return getNetworkConfig(getCurrentNetwork());
}

// ─── View helpers ─────────────────────────────────────────────────────────────

/**
 * Simulate a read-only contract call and return the raw ScVal result.
 * Uses a throwaway keypair as the source — no signing required.
 */
async function simulateView(
  functionName: string,
  args: import("@stellar/stellar-sdk").xdr.ScVal[]
) {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(contract.call(functionName, ...args))
    .setTimeout(30)
    .build();

  const result = await server.simulateTransaction(tx);

  if (StellarSdk.SorobanRpc.Api.isSimulationError(result)) {
    throw new Error(`Simulation failed: ${result.error}`);
  }

  return result.result?.retval;
}

/**
 * Decode a Soroban ScVal (Swap struct) into a plain JS object using scValToNative.
 *
 * scValToNative converts:
 *   - u64  → BigInt
 *   - i128 → BigInt
 *   - Address → string (G...)
 *   - Map  → object
 *   - Vec  → array
 *   - Bytes → Buffer
 *   - Enum variant → { tag: string, values: [...] }
 */
function decodeSwapScVal(
  scVal: import("@stellar/stellar-sdk").xdr.ScVal | undefined,
  swapId: number
) {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

  // SwapStatus enum: scValToNative returns { tag: "Pending"|"Completed"|"Cancelled" }
  const status =
    typeof native.status === "object" && native.status !== null
      ? native.status.tag ?? "Unknown"
      : String(native.status ?? "Unknown");

  // decryption_key is Option<Bytes>: scValToNative returns null or Buffer
  let decryptionKey = null;
  if (
    native.decryption_key instanceof Uint8Array ||
    Buffer.isBuffer(native.decryption_key)
  ) {
    decryptionKey = Buffer.from(native.decryption_key).toString("hex");
  }

  const usdcAmount = Number(native.usdc_amount ?? 0);
  if (usdcAmount <= 0) {
    console.warn(
      `[decodeSwapScVal] swap ${swapId} has non-positive usdc_amount: ${usdcAmount}`
    );
  }

  // hold_until is Option<u64>: scValToNative returns null or BigInt.
  const holdUntil =
    native.hold_until === null || native.hold_until === undefined
      ? null
      : Number(native.hold_until);

  return {
    id: swapId,
    listing_id: Number(native.listing_id ?? 0),
    buyer: String(native.buyer ?? ""),
    seller: String(native.seller ?? ""),
    usdc_amount: usdcAmount,
    usdc_token: String(native.usdc_token ?? ""),
    created_at: Number(native.created_at ?? 0),
    expires_at: Number(native.expires_at ?? 0),
    status,
    decryption_key: decryptionKey,
    hold_until: holdUntil,
    buyer_confirmed: Boolean(native.buyer_confirmed),
  };
}

/**
 * Fetch all swap IDs for a buyer by calling get_swaps_by_buyer.
 * @param {string} buyerAddress - Stellar public key (G...)
 * @returns {Promise<number[]>}
 */
export async function getSwapsByBuyer(buyerAddress: string) {
  const addressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(buyerAddress),
    { type: "address" }
  );

  const retval = await simulateView("get_swaps_by_buyer", [addressScVal]);
  if (!retval) return [];

  // scValToNative on Vec<u64> returns BigInt[]
  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map((v) => Number(v));
}

/**
 * Fetch full swap details for a single swap ID using get_swap contract function.
 * Task 1: single call replaces multiple get_swap_status + get_decryption_key calls.
 * @param {number} swapId
 * @returns {Promise<object|null>}
 */
export async function getSwap(swapId: number) {
  const swapIdScVal = StellarSdk.nativeToScVal(swapId, { type: "u64" });
  const retval = await simulateView("get_swap", [swapIdScVal]);
  return decodeSwapScVal(retval, swapId);
}

/**
 * Fetch the current ledger timestamp (unix seconds).
 * @returns {Promise<number>}
 */
export async function getLedgerTimestamp(): Promise<number> {
  return Math.floor(Date.now() / 1000);
}

// ─── Mutations ────────────────────────────────────────────────────────────────

/**
 * Calls cancel_swap(swap_id) on the atomic_swap contract.
 * @param {string} swapId - The swap ID (u64 as string or number)
 * @param {object} wallet  - Connected wallet with signTransaction method
 * @returns {Promise<void>}
 */
export async function cancelSwap(
  swapId: number | string,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  }
) {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "cancel_swap",
        StellarSdk.nativeToScVal(Number(swapId), { type: "u64" })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

/**
 * Encode a ProofNode[] as a Soroban Vec<ProofNode> ScVal.
 *
 * Expected JSON format for each node:
 *   { "sibling": "0x..." or hex string (32 bytes), "is_left": true|false }
 */
function encodeProofPath(
  proofPath: ProofNode[]
): import("@stellar/stellar-sdk").xdr.ScVal {
  return StellarSdk.xdr.ScVal.scvVec(
    proofPath.map((node) => {
      const siblingBytes = Buffer.from(node.sibling.replace(/^0x/, ""), "hex");
      if (siblingBytes.length !== 32) {
        throw new Error(
          `ProofNode sibling must be exactly 32 bytes (64 hex chars), got ${siblingBytes.length} bytes.`
        );
      }
      return StellarSdk.xdr.ScVal.scvMap([
        new StellarSdk.xdr.ScMapEntry({
          key: StellarSdk.xdr.ScVal.scvSymbol("is_left"),
          val: StellarSdk.xdr.ScVal.scvBool(node.is_left),
        }),
        new StellarSdk.xdr.ScMapEntry({
          key: StellarSdk.xdr.ScVal.scvSymbol("sibling"),
          val: StellarSdk.xdr.ScVal.scvBytes(siblingBytes),
        }),
      ]);
    })
  );
}

/**
 * Calls confirm_swap(swap_id, decryption_key, proof_path) on the atomic_swap contract.
 *
 * proof_path format (JSON array):
 *   [
 *     { "sibling": "<64-char-hex>", "is_left": true },
 *     { "sibling": "<64-char-hex>", "is_left": false },
 *     ...
 *   ]
 *
 * Each sibling must be exactly 32 bytes (64 hex characters).
 *
 * @param {string|number} swapId
 * @param {string} decryptionKey - hex or base64 string of the decryption key
 * @param {ProofNode[]} proofPath - Merkle proof path (Vec<ProofNode>)
 * @param {object} wallet        - { address, signTransaction }
 */
export async function confirmSwap(
  swapId: number | string,
  decryptionKey: string,
  proofPath: ProofNode[],
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  }
) {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }
  if (!decryptionKey || !decryptionKey.trim()) {
    throw new Error("Decryption key is required.");
  }
  if (!proofPath || proofPath.length === 0) {
    throw new Error("Proof path is required and must be non-empty.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const keyBytes = StellarSdk.xdr.ScVal.scvBytes(
    Buffer.from(decryptionKey.replace(/^0x/, ""), "hex")
  );

  const proofPathScVal = encodeProofPath(proofPath);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "confirm_swap",
        StellarSdk.nativeToScVal(Number(swapId), { type: "u64" }),
        keyBytes,
        proofPathScVal
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

// ─── Shared submit helper ─────────────────────────────────────────────────────

export async function approveUsdc(
  usdcContractId: string,
  spenderId: string,
  amount: bigint,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  }
) {
  const cfg = currentConfig();
  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(usdcContractId);
  const spenderAddressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(spenderId),
    { type: "address" }
  );
  const fromAddressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(wallet.address),
    { type: "address" }
  );

  const ledgerResponse = await server.getLatestLedger();
  const expirationLedger = ledgerResponse.sequence + 100;

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "approve",
        fromAddressScVal,
        spenderAddressScVal,
        StellarSdk.nativeToScVal(amount, { type: "i128" }),
        StellarSdk.nativeToScVal(expirationLedger, { type: "u32" })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function initiateSwap(
  listingId: number,
  sellerAddress: string,
  usdcContractId: string,
  usdcAmount: bigint,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  }
): Promise<number> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "initiate_swap",
        StellarSdk.nativeToScVal(listingId, { type: "u64" }), // listing_id
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        }), // buyer
        StellarSdk.nativeToScVal(new StellarSdk.Address(sellerAddress), {
          type: "address",
        }), // seller
        StellarSdk.nativeToScVal(new StellarSdk.Address(usdcContractId), {
          type: "address",
        }), // usdc_token
        StellarSdk.nativeToScVal(usdcAmount, { type: "i128" }) // usdc_amount
      )
    )
    .setTimeout(30)
    .build();

  const txResponse = await submitAndPoll(tx, wallet, server);

  if (!txResponse.returnValue) {
    throw new Error("Transaction succeeded but returned no value (swap ID expected).");
  }

  // Contract returns u64 swap ID. scValToNative returns BigInt for u64.
  const swapId = StellarSdk.scValToNative(txResponse.returnValue);
  return Number(swapId);
}

async function submitAndPoll(
  tx: import("@stellar/stellar-sdk").Transaction,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  },
  server: import("@stellar/stellar-sdk").SorobanRpc.Server
): Promise<StellarSdk.SorobanRpc.Api.GetSuccessfulTransactionResponse> {
  const cfg = currentConfig();
  const preparedTx = await server.prepareTransaction(tx);
  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(
    signedXdr,
    cfg.passphrase
  );

  const sendResult = await server.sendTransaction(signedTx);
  if (sendResult.status === "ERROR") {
    throw new Error(`Transaction failed: ${sendResult.errorResult}`);
  }

  // Poll until the transaction leaves NOT_FOUND state
  let txResponse = await server.getTransaction(sendResult.hash);
  while (txResponse.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1500));
    txResponse = await server.getTransaction(sendResult.hash);
  }

  if (txResponse.status !== "SUCCESS") {
    throw new Error(`Transaction did not succeed: ${txResponse.status}`);
  }

  return txResponse as StellarSdk.SorobanRpc.Api.GetSuccessfulTransactionResponse;
}

// ─── IP Registry ──────────────────────────────────────────────────────────────

/**
 * Simulate a read-only call against the ip_registry contract.
 */
async function simulateIpRegistryView(
  functionName: string,
  args: import("@stellar/stellar-sdk").xdr.ScVal[]
) {
  const cfg = currentConfig();
  if (!cfg.ipRegistry) {
    throw new Error("VITE_CONTRACT_IP_REGISTRY is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(cfg.ipRegistry);

  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(contract.call(functionName, ...args))
    .setTimeout(30)
    .build();

  const result = await server.simulateTransaction(tx);

  if (StellarSdk.SorobanRpc.Api.isSimulationError(result)) {
    throw new Error(`Simulation failed: ${result.error}`);
  }

  return result.result?.retval;
}

/**
 * Decode a Listing ScVal into a plain JS object.
 * Listing { owner, ipfs_hash, merkle_root, royalty_bps, royalty_recipient, price_usdc }
 */
function decodeListingScVal(
  scVal: import("@stellar/stellar-sdk").xdr.ScVal | undefined,
  listingId: number
) {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

  // ipfs_hash and merkle_root are Bytes — scValToNative returns Buffer/Uint8Array
  const toHex = (v: any) =>
    v instanceof Uint8Array || Buffer.isBuffer(v)
      ? Buffer.from(v).toString("hex")
      : String(v ?? "");

  return {
    id: listingId,
    owner: String(native.owner ?? ""),
    ipfs_hash: toHex(native.ipfs_hash),
    merkle_root: toHex(native.merkle_root),
    royalty_bps: Number(native.royalty_bps ?? 0),
    royalty_recipient: String(native.royalty_recipient ?? ""),
    price_usdc: Number(native.price_usdc ?? 0),
  };
}

/**
 * Fetch all listing IDs owned by the given address.
 * @param {string} ownerAddress - Stellar public key (G...)
 * @returns {Promise<number[]>}
 */
export async function getListingsByOwner(ownerAddress: string) {
  const addressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(ownerAddress),
    { type: "address" }
  );

  const retval = await simulateIpRegistryView("list_by_owner", [addressScVal]);
  if (!retval) return [];

  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map((v) => Number(v));
}

/**
 * Fetch full listing details for a single listing ID.
 * @param {number} listingId
 * @returns {Promise<object|null>}
 */
export async function getListing(listingId: number) {
  const retval = await simulateIpRegistryView("get_listing", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
  ]);

  if (!retval) return null;
  return decodeListingScVal(retval, listingId);
}

/**
 * Fetch the current number of listings stored in ip_registry.
 * @returns {Promise<number>}
 */
export async function getListingCount() {
  const retval = await simulateIpRegistryView("listing_count", []);
  if (!retval) return 0;
  return Number(StellarSdk.scValToNative(retval) ?? 0);
}

/**
 * Decode a Version ScVal into a plain JS object.
 * Version { version_number, timestamp, changelog, ipfs_hash, merkle_root,
 *           price_usdc, royalty_bps, created_by }
 */
function decodeVersionScVal(native: any): IpVersion {
  const toHex = (v: any) =>
    v instanceof Uint8Array || Buffer.isBuffer(v)
      ? Buffer.from(v).toString("hex")
      : String(v ?? "");
  const toUtf8 = (v: any) =>
    v instanceof Uint8Array || Buffer.isBuffer(v)
      ? Buffer.from(v).toString("utf-8")
      : String(v ?? "");

  return {
    version_number: Number(native.version_number ?? 0),
    timestamp: Number(native.timestamp ?? 0),
    changelog: toUtf8(native.changelog),
    ipfs_hash: toHex(native.ipfs_hash),
    merkle_root: toHex(native.merkle_root),
    price_usdc: Number(native.price_usdc ?? 0),
    royalty_bps: Number(native.royalty_bps ?? 0),
    created_by: String(native.created_by ?? ""),
  };
}

/**
 * Fetch one bounded page of a listing's version history, oldest first.
 * Mirrors the contract's `get_version_history_page(listing_id, offset, limit)`,
 * which caps `limit` server-side — callers should page with `offset` rather
 * than requesting the whole history in one call.
 *
 * @param {number} listingId
 * @param {number} offset - 0-based index into the version sequence
 * @param {number} limit - max entries to return (server-capped)
 * @returns {Promise<IpVersion[]>}
 */
export async function getVersionHistoryPage(
  listingId: number,
  offset: number,
  limit: number
): Promise<IpVersion[]> {
  const retval = await simulateIpRegistryView("get_version_history_page", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
    StellarSdk.nativeToScVal(offset, { type: "u32" }),
    StellarSdk.nativeToScVal(limit, { type: "u32" }),
  ]);

  if (!retval) return [];
  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map(decodeVersionScVal);
}

/**
 * Return whether a listing currently has a pending swap in atomic_swap.
 * @param {number} listingId
 * @returns {Promise<boolean>}
 */
export async function hasPendingSwap(listingId: number) {
  const retval = await simulateView("has_pending_swap", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
  ]);
  return Boolean(retval && StellarSdk.scValToNative(retval)) ?? false;
}

/**
 * Register a new IP listing on the ip_registry contract.
 * Calls register_ip(owner, ipfs_hash, merkle_root, royalty_bps, royalty_recipient, price_usdc)
 *
 * @param ipfsHash         - IPFS content hash (hex string)
 * @param merkleRoot       - Merkle root (hex string, typically 64-char)
 * @param royaltyBps       - Royalty basis points (0-10000, where 10000 = 100%)
 * @param royaltyRecipient - Stellar address receiving royalties (G...)
 * @param priceUsdc        - Price in USDC (human-readable, e.g. 10.5)
 * @param wallet           - Connected wallet { address, signTransaction }
 * @returns Promise<void>
 */
export async function registerIp(
  ipfsHash: string,
  merkleRoot: string,
  royaltyBps: number,
  royaltyRecipient: string,
  priceUsdc: number,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.ipRegistry) {
    throw new Error("VITE_CONTRACT_IP_REGISTRY is not configured.");
  }
  if (!ipfsHash || !ipfsHash.trim()) {
    throw new Error("IPFS hash is required.");
  }
  if (!merkleRoot || !merkleRoot.trim()) {
    throw new Error("Merkle root is required.");
  }
  if (royaltyBps < 0 || royaltyBps > 10000) {
    throw new Error("Royalty bps must be between 0 and 10000.");
  }
  if (priceUsdc <= 0) {
    throw new Error("Price must be greater than 0.");
  }
  if (!royaltyRecipient || !royaltyRecipient.trim()) {
    throw new Error("Royalty recipient address is required.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.ipRegistry);

  // Convert hex strings to Bytes (Buffer)
  const ipfsBytes = Buffer.from(ipfsHash.replace(/^0x/, ""), "hex");
  const merkleBytes = Buffer.from(merkleRoot.replace(/^0x/, ""), "hex");

  // USDC has 7 decimals, price_usdc is i128
  const priceRaw = Math.round(priceUsdc * 1e7);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "register_ip",
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        }),
        StellarSdk.xdr.ScVal.scvBytes(ipfsBytes),
        StellarSdk.xdr.ScVal.scvBytes(merkleBytes),
        StellarSdk.nativeToScVal(royaltyBps, { type: "u32" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(royaltyRecipient), {
          type: "address",
        }),
        StellarSdk.nativeToScVal(priceRaw, { type: "i128" })
      )
    )
    .setTimeout(60)
    .build();

  await submitAndPoll(tx, wallet, server);
}

/**
 * Fetch all swap IDs for a seller by calling get_swaps_by_seller.
 * @param {string} sellerAddress - Stellar public key (G...)
 * @returns {Promise<number[]>}
 */
export async function getSwapsBySeller(sellerAddress: string) {
  const addressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(sellerAddress),
    { type: "address" }
  );

  const retval = await simulateView("get_swaps_by_seller", [addressScVal]);
  if (!retval) return [];

  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map((v) => Number(v));
}

// ─── USDC Balance ─────────────────────────────────────────────────────────────

export { USDC_DECIMALS } from "./types";

/**
 * Fetch the USDC balance for a given address by calling `balance(address)`
 * on the USDC token contract.
 * @param {string} address - Stellar public key (G...)
 * @returns {Promise<number>} - Balance in human-readable USDC (e.g. 12.5)
 */
export async function getUsdcBalance(address: string): Promise<number> {
  const cfg = currentConfig();
  if (!cfg.usdc) return 0;

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(cfg.usdc);

  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "balance",
        StellarSdk.nativeToScVal(new StellarSdk.Address(address), {
          type: "address",
        })
      )
    )
    .setTimeout(30)
    .build();

  const result = await server.simulateTransaction(tx);
  if (StellarSdk.SorobanRpc.Api.isSimulationError(result)) return 0;

  const retval = result.result?.retval;
  if (!retval) return 0;

  const raw = StellarSdk.scValToNative(retval);
  return Number(raw) / Math.pow(10, USDC_DECIMALS);
}

// ─── ZK Verifier ──────────────────────────────────────────────────────────────


async function simulateZkView(
  functionName: string,
  args: import("@stellar/stellar-sdk").xdr.ScVal[]
) {
  const cfg = currentConfig();
  if (!cfg.zkVerifier)
    throw new Error("VITE_CONTRACT_ZK_VERIFIER is not configured.");
  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(cfg.zkVerifier);
  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(contract.call(functionName, ...args))
    .setTimeout(30)
    .build();
  const result = await server.simulateTransaction(tx);
  if (StellarSdk.SorobanRpc.Api.isSimulationError(result)) {
    throw new Error(`Simulation failed: ${result.error}`);
  }
  return result.result?.retval;
}

/**
 * Call set_merkle_root on the zk_verifier contract.
 * @param listingId - listing ID (u64)
 * @param rootHex   - 32-byte Merkle root as a 64-char hex string
 * @param wallet    - connected wallet
 */
export async function setMerkleRoot(
  listingId: number,
  rootHex: string,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.zkVerifier)
    throw new Error("VITE_CONTRACT_ZK_VERIFIER is not configured.");
  const rootBytes = Buffer.from(rootHex.replace(/^0x/, ""), "hex");
  if (rootBytes.length !== 32)
    throw new Error("Root must be exactly 32 bytes (64 hex chars).");

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.zkVerifier);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "set_merkle_root",
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        }),
        StellarSdk.nativeToScVal(listingId, { type: "u64" }),
        StellarSdk.xdr.ScVal.scvBytes(rootBytes)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

// ─── Dispute resolution ───────────────────────────────────────────────────────

export interface DisputeRecord {
  swap_id: number;
  raised_by: string;
  raised_at_ledger: number;
  evidence_count: number;
  outcome: "Pending" | "FavorBuyer" | "FavorSeller";
  resolved_at_ledger: number | null;
  vote_weight_buyer: string;
  vote_weight_seller: string;
  commit_deadline_ledger: number;
  reveal_deadline_ledger: number;
  appeal_deadline_ledger: number | null;
  is_appealed: boolean;
}

export interface EvidenceItem {
  submitter: string;
  ipfs_hash: string;
  submitted_at_ledger: number;
}

function decodeDisputeScVal(
  scVal: import("@stellar/stellar-sdk").xdr.ScVal | undefined
): DisputeRecord | null {
  if (!scVal || scVal.switch().name === "scvVoid") return null;
  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

  const outcome =
    typeof native.outcome === "object" && native.outcome !== null
      ? (native.outcome.tag as "Pending" | "FavorBuyer" | "FavorSeller")
      : "Pending";

  const toNum = (v: any) => (v != null ? Number(v) : null);

  return {
    swap_id: Number(native.swap_id ?? 0),
    raised_by: String(native.raised_by ?? ""),
    raised_at_ledger: Number(native.raised_at_ledger ?? 0),
    evidence_count: Number(native.evidence_count ?? 0),
    outcome,
    resolved_at_ledger: toNum(native.resolved_at_ledger),
    vote_weight_buyer: String(native.vote_weight_buyer ?? "0"),
    vote_weight_seller: String(native.vote_weight_seller ?? "0"),
    commit_deadline_ledger: Number(native.commit_deadline_ledger ?? 0),
    reveal_deadline_ledger: Number(native.reveal_deadline_ledger ?? 0),
    appeal_deadline_ledger: toNum(native.appeal_deadline_ledger),
    is_appealed: Boolean(native.is_appealed),
  };
}

export async function getDispute(swapId: number): Promise<DisputeRecord | null> {
  const retval = await simulateView("get_dispute", [
    StellarSdk.nativeToScVal(swapId, { type: "u64" }),
  ]);
  return decodeDisputeScVal(retval);
}

export async function getEvidence(
  swapId: number,
  index: number
): Promise<EvidenceItem | null> {
  const retval = await simulateView("get_evidence", [
    StellarSdk.nativeToScVal(swapId, { type: "u64" }),
    StellarSdk.nativeToScVal(index, { type: "u32" }),
  ]);
  if (!retval || retval.switch().name === "scvVoid") return null;
  const native = StellarSdk.scValToNative(retval);
  if (!native || typeof native !== "object") return null;
  const hashRaw = native.ipfs_hash;
  const ipfsHash =
    hashRaw instanceof Uint8Array || Buffer.isBuffer(hashRaw)
      ? Buffer.from(hashRaw).toString("utf8")
      : String(hashRaw ?? "");
  return {
    submitter: String(native.submitter ?? ""),
    ipfs_hash: ipfsHash,
    submitted_at_ledger: Number(native.submitted_at_ledger ?? 0),
  };
}

export async function getArbiters(): Promise<string[]> {
  const retval = await simulateView("get_arbiters", []);
  if (!retval) return [];
  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map(String);
}

export async function getCurrentLedger(): Promise<number> {
  const cfg = currentConfig();
  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const ledger = await server.getLatestLedger();
  return ledger.sequence;
}

export async function raiseDispute(
  swapId: number,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap)
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "raise_dispute",
        StellarSdk.nativeToScVal(swapId, { type: "u64" })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function submitEvidence(
  swapId: number,
  ipfsHash: string,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap)
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const hashBytes = Buffer.from(ipfsHash, "utf8");

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "submit_evidence",
        StellarSdk.nativeToScVal(swapId, { type: "u64" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        }),
        StellarSdk.xdr.ScVal.scvBytes(hashBytes)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

/**
 * Compute the commitment hash for the commitment-reveal voting pattern.
 * commitment = sha256(vote_byte || utf8(salt))
 * Returns a 32-byte Buffer.
 */
export async function computeVoteCommitment(
  favorBuyer: boolean,
  salt: string
): Promise<Buffer> {
  const voteByte = favorBuyer ? 0x01 : 0x00;
  const saltBytes = Buffer.from(salt, "utf8");
  const preimage = Buffer.concat([Buffer.from([voteByte]), saltBytes]);

  const hashBuffer = await crypto.subtle.digest("SHA-256", preimage);
  return Buffer.from(hashBuffer);
}

export async function commitVote(
  swapId: number,
  favorBuyer: boolean,
  salt: string,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap)
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const commitment = await computeVoteCommitment(favorBuyer, salt);

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "commit_vote",
        StellarSdk.nativeToScVal(swapId, { type: "u64" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        }),
        StellarSdk.xdr.ScVal.scvBytes(commitment)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function revealVote(
  swapId: number,
  favorBuyer: boolean,
  salt: string,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap)
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const saltBytes = Buffer.from(salt, "utf8");

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "reveal_vote",
        StellarSdk.nativeToScVal(swapId, { type: "u64" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        }),
        StellarSdk.xdr.ScVal.scvBool(favorBuyer),
        StellarSdk.xdr.ScVal.scvBytes(saltBytes)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function finalizeDispute(
  swapId: number,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap)
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "finalize_dispute",
        StellarSdk.nativeToScVal(swapId, { type: "u64" })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function appealDispute(
  swapId: number,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap)
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "appeal_dispute",
        StellarSdk.nativeToScVal(swapId, { type: "u64" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export interface Listing {
  id: number;
  owner: string;
  ipfs_hash: string;
  merkle_root: string;
  royalty_bps: number;
  royalty_recipient: string;
  price_usdc: number;
}

export interface ProofNode {
  sibling: string; // 32-byte hex
  is_left: boolean;
}

/**
 * Call verify_partial_proof on the zk_verifier contract (simulation only).
 * @param listingId - listing ID (u64)
 * @param leafHex   - leaf data as hex string
 * @param path      - array of ProofNode
 * @returns boolean
 */
export async function verifyPartialProof(
  listingId: number,
  leafHex: string,
  path: ProofNode[]
): Promise<boolean> {
  const leafBytes = Buffer.from(leafHex.replace(/^0x/, ""), "hex");

  const pathScVal = encodeProofPath(path);

  const retval = await simulateZkView("verify_partial_proof", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
    StellarSdk.xdr.ScVal.scvBytes(leafBytes),
    pathScVal,
  ]);

  if (!retval) return false;
  const native = StellarSdk.scValToNative(retval);
  return Boolean(native);
}

// ─── Admin Functions ─────────────────────────────────────────────────────────────

export async function pauseAtomicSwap(
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);
  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(contract.call("pause"))
    .setTimeout(30)
    .build();
  await submitAndPoll(tx, wallet, server);
}

export async function unpauseAtomicSwap(
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);
  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(contract.call("unpause"))
    .setTimeout(30)
    .build();
  await submitAndPoll(tx, wallet, server);
}

export async function updateAtomicSwapConfig(
  feeBps: number,
  feeRecipient: string,
  cancelDelaySecs: number,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  
  // Input validation
  if (feeBps < 0 || feeBps > 10000) {
    throw new Error("Fee basis points must be between 0 and 10000 (0-100%)");
  }
  if (!feeRecipient || !feeRecipient.trim()) {
    throw new Error("Fee recipient address is required");
  }
  if (!feeRecipient.startsWith('G') || feeRecipient.length !== 56) {
    throw new Error("Invalid Stellar address format");
  }
  if (cancelDelaySecs < 0) {
    throw new Error("Cancel delay must be non-negative");
  }
  
  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);
  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "update_config",
        StellarSdk.nativeToScVal(feeBps, { type: "u32" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(feeRecipient), { type: "address" }),
        StellarSdk.nativeToScVal(cancelDelaySecs, { type: "u64" })
      )
    )
    .setTimeout(30)
    .build();
  await submitAndPoll(tx, wallet, server);
}

export async function getAtomicSwapConfig(): Promise<any> {
  const retval = await simulateView("get_config", []);
  if (!retval) return null;
  return StellarSdk.scValToNative(retval);
}

export async function isAtomicSwapPaused(): Promise<boolean> {
  const retval = await simulateView("is_paused", []);
  if (!retval) return false;
  return Boolean(StellarSdk.scValToNative(retval));
}

// ─── Multi-sig approval ───────────────────────────────────────────────────────

import type { MultiSigConfig, MultiSigApproval } from "./multiSigTypes";

/**
 * Fetch the current multi-sig configuration from the atomic_swap contract.
 * Returns null if multi-sig has not been configured yet.
 */
export async function getMultiSigConfig(): Promise<MultiSigConfig | null> {
  const retval = await simulateView("get_multisig_config_view", []);
  if (!retval || retval.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(retval);
  if (!native || typeof native !== "object") return null;

  const signersArr = Array.isArray(native.signers) ? native.signers : [];
  return {
    threshold: String(native.threshold ?? "0"),
    signers: signersArr.map(String),
    required_approvals: Number(native.required_approvals ?? 2),
    enabled: Boolean(native.enabled),
  };
}

/**
 * Fetch the multi-sig approval accumulator for a given swap ID.
 * Returns null if the swap was not subject to multi-sig.
 */
export async function getMultiSigApproval(
  swapId: number
): Promise<MultiSigApproval | null> {
  const retval = await simulateView("get_multisig_approval", [
    StellarSdk.nativeToScVal(swapId, { type: "u64" }),
  ]);
  if (!retval || retval.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(retval);
  if (!native || typeof native !== "object") return null;

  const approvedBy = Array.isArray(native.approved_by)
    ? native.approved_by.map(String)
    : [];
  return {
    swap_id: Number(native.swap_id ?? swapId),
    approved_by: approvedBy,
    nonce: Number(native.nonce ?? 0),
  };
}

/**
 * Signer: approve a high-value swap that is awaiting multi-sig sign-off.
 * Requires the caller to be a configured signer on the contract.
 *
 * @param swapId - ID of the swap to approve
 * @param wallet - Connected wallet (must be a configured multi-sig signer)
 */
export async function approveMultiSigSwap(
  swapId: number,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "approve_multisig_swap",
        StellarSdk.nativeToScVal(swapId, { type: "u64" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), {
          type: "address",
        })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

/**
 * Admin: update the multi-sig configuration on the atomic_swap contract.
 *
 * @param threshold         - USDC threshold (human-readable, e.g. 10000 for 10,000 USDC)
 * @param signers           - Array of Stellar addresses authorised to approve
 * @param requiredApprovals - Minimum approvals needed (2 for 2-of-2 or 2-of-3)
 * @param enabled           - Toggle; false disables the gate without clearing config
 * @param wallet            - Admin wallet
 */
export async function setMultiSigConfig(
  threshold: number,
  signers: string[],
  requiredApprovals: number,
  enabled: boolean,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  }
): Promise<void> {
  const cfg = currentConfig();
  if (!cfg.atomicSwap) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }
  if (signers.length === 0) {
    throw new Error("At least one signer is required.");
  }
  if (requiredApprovals < 1 || requiredApprovals > signers.length) {
    throw new Error(
      `required_approvals must be between 1 and ${signers.length}.`
    );
  }

  const server = new StellarSdk.SorobanRpc.Server(cfg.rpcUrl);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(cfg.atomicSwap);

  // Encode signers as Vec<Address>
  const signersScVal = StellarSdk.xdr.ScVal.scvVec(
    signers.map((addr) =>
      StellarSdk.nativeToScVal(new StellarSdk.Address(addr), {
        type: "address",
      })
    )
  );

  // threshold is in human-readable USDC — convert to 7-decimal i128
  const thresholdRaw = BigInt(Math.round(threshold * Math.pow(10, USDC_DECIMALS)));

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: cfg.passphrase,
  })
    .addOperation(
      contract.call(
        "set_multisig_config",
        StellarSdk.nativeToScVal(thresholdRaw, { type: "i128" }),
        signersScVal,
        StellarSdk.nativeToScVal(requiredApprovals, { type: "u32" }),
        StellarSdk.xdr.ScVal.scvBool(enabled)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}
