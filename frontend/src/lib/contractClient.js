import * as StellarSdk from "@stellar/stellar-sdk";

const RPC_URL =
  import.meta.env.VITE_STELLAR_RPC_URL ||
  "https://soroban-testnet.stellar.org";

const ATOMIC_SWAP_CONTRACT_ID = import.meta.env.VITE_CONTRACT_ATOMIC_SWAP;

const networkPassphrase = () =>
  import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? StellarSdk.Networks.PUBLIC
    : StellarSdk.Networks.TESTNET;

// ─── View helpers ─────────────────────────────────────────────────────────────

/**
 * Simulate a read-only contract call and return the raw ScVal result.
 * Uses a throwaway keypair as the source — no signing required.
 */
async function simulateView(functionName, args) {
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
function decodeSwapScVal(scVal, swapId) {
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
export async function getSwapsByBuyer(buyerAddress) {
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
 * Fetch full swap details for a single swap ID.
 * Reads the Swap struct from contract storage via getLedgerEntries.
 * @param {number} swapId
 * @returns {Promise<object|null>}
 */
export async function getSwap(swapId) {
  if (!ATOMIC_SWAP_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);

  // Build the DataKey::Swap(swapId) storage key
  // DataKey::Swap(u64) encodes as a Vec<ScVal> = [Symbol("Swap"), u64]
  const dataKey = StellarSdk.xdr.ScVal.scvVec([
    StellarSdk.xdr.ScVal.scvSymbol("Swap"),
    StellarSdk.nativeToScVal(swapId, { type: "u64" }),
  ]);

  const contractId = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID).contractId();

  const ledgerKey = StellarSdk.xdr.LedgerKey.contractData(
    new StellarSdk.xdr.LedgerKeyContractData({
      contract: new StellarSdk.Address(contractId).toScAddress(),
      key: dataKey,
      durability: StellarSdk.xdr.ContractDataDurability.persistent(),
    })
  );

  const response = await server.getLedgerEntries(ledgerKey);

  if (!response.entries || response.entries.length === 0) return null;

  const entry = response.entries[0];
  const contractData = entry.val.contractData();
  const swapScVal = contractData.val();

  return decodeSwapScVal(swapScVal, swapId);
}

/**
 * Fetch the current ledger timestamp (unix seconds).
 * @returns {Promise<number>}
 */
export async function getLedgerTimestamp() {
  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const latest = await server.getLatestLedger();
  // getLatestLedger returns { id, protocolVersion, sequence }
  // We approximate timestamp from sequence (not exact but sufficient for UI)
  // Real approach: use server.getNetwork() or a known genesis timestamp
  // For now return Date.now() / 1000 as a safe fallback
  return Math.floor(Date.now() / 1000);
}

// ─── Mutations ────────────────────────────────────────────────────────────────

/**
 * Calls cancel_swap(swap_id) on the atomic_swap contract.
 * @param {string} swapId - The swap ID (u64 as string or number)
 * @param {object} wallet  - Connected wallet with signTransaction method
 * @returns {Promise<void>}
 */
export async function cancelSwap(swapId, wallet) {
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
export async function confirmSwap(swapId, decryptionKey, wallet) {
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

async function submitAndPoll(tx, wallet, server) {
  const preparedTx = await server.prepareTransaction(tx);

  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(signedXdr, networkPassphrase());

  const result = await server.sendTransaction(signedTx);
  if (result.status === "ERROR") {
    throw new Error(`Transaction failed: ${result.errorResult}`);
  }

  let response = result;
  while (response.status === "PENDING" || response.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1500));
    response = await server.getTransaction(result.hash);
  }

  if (response.status !== "SUCCESS") {
    throw new Error(`Transaction did not succeed: ${response.status}`);
  }
}
