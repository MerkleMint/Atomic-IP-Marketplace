import * as StellarSdk from "@stellar/stellar-sdk";

const RPC_URL =
  import.meta.env.VITE_STELLAR_RPC_URL ||
  "https://soroban-testnet.stellar.org";

const ATOMIC_SWAP_CONTRACT_ID = import.meta.env.VITE_CONTRACT_ATOMIC_SWAP;
const IP_REGISTRY_CONTRACT_ID = import.meta.env.VITE_CONTRACT_IP_REGISTRY;

const networkPassphrase = () =>
  import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? StellarSdk.Networks.PUBLIC
    : StellarSdk.Networks.TESTNET;

// ─── View helpers ─────────────────────────────────────────────────────────────

/**
 * Simulate a read-only contract call and return the raw ScVal result.
 * Uses a throwaway keypair as the source — no signing required.
 */
async function simulateView(functionName: string, args: import("@stellar/stellar-sdk").xdr.ScVal[]) {
  if (!ATOMIC_SWAP_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID);

  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
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
function decodeSwapScVal(scVal: import("@stellar/stellar-sdk").xdr.ScVal | undefined, swapId: number) {
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
  if (native.decryption_key instanceof Uint8Array || Buffer.isBuffer(native.decryption_key)) {
    decryptionKey = Buffer.from(native.decryption_key).toString("hex");
  }

  return {
    id: swapId,
    listing_id: Number(native.listing_id ?? 0),
    buyer: String(native.buyer ?? ""),
    seller: String(native.seller ?? ""),
    usdc_amount: Number(native.usdc_amount ?? 0),
    created_at: Number(native.created_at ?? 0),
    expires_at: Number(native.expires_at ?? 0),
    status,
    decryption_key: decryptionKey,
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
export async function cancelSwap(swapId: number | string, wallet: {address:string; signTransaction:(xdr:string)=>Promise<string>}) {
  if (!ATOMIC_SWAP_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
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
 * Calls confirm_swap(swap_id, decryption_key) on the atomic_swap contract.
 * @param {string|number} swapId
 * @param {string} decryptionKey - hex or base64 string of the decryption key
 * @param {object} wallet        - { address, signTransaction }
 */
export async function confirmSwap(swapId: number | string, decryptionKey: string, wallet: {address:string; signTransaction:(xdr:string)=>Promise<string>}) {
  if (!ATOMIC_SWAP_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }
  if (!decryptionKey || !decryptionKey.trim()) {
    throw new Error("Decryption key is required.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID);

  const keyBytes = StellarSdk.xdr.ScVal.scvBytes(
    Buffer.from(decryptionKey.replace(/^0x/, ""), "hex")
  );

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "confirm_swap",
        StellarSdk.nativeToScVal(Number(swapId), { type: "u64" }),
        keyBytes
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

// ─── Shared submit helper ─────────────────────────────────────────────────────

async function submitAndPoll(
  tx: import("@stellar/stellar-sdk").Transaction,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> },
  server: import("@stellar/stellar-sdk").SorobanRpc.Server
) {
  const preparedTx = await server.prepareTransaction(tx);
  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(signedXdr, networkPassphrase());

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
}

// ─── IP Registry ──────────────────────────────────────────────────────────────

/**
 * Simulate a read-only call against the ip_registry contract.
 */
async function simulateIpRegistryView(functionName, args) {
  if (!IP_REGISTRY_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_IP_REGISTRY is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(IP_REGISTRY_CONTRACT_ID);

  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
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
function decodeListingScVal(scVal, listingId) {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

  // ipfs_hash and merkle_root are Bytes — scValToNative returns Buffer/Uint8Array
  const toHex = (v) =>
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
export async function getListingsByOwner(ownerAddress) {
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
export async function getListing(listingId) {
  const retval = await simulateIpRegistryView("get_listing", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
  ]);

  if (!retval) return null;
  return decodeListingScVal(retval, listingId);
}

/**
 * Fetch all swap IDs for a seller by calling get_swaps_by_seller.
 * @param {string} sellerAddress - Stellar public key (G...)
 * @returns {Promise<number[]>}
 */
export async function getSwapsBySeller(sellerAddress) {
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
