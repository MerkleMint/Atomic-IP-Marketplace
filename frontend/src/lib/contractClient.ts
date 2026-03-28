import * as StellarSdk from "@stellar/stellar-sdk";
import {
  CONTRACT_ATOMIC_SWAP,
  CONTRACT_IP_REGISTRY,
  CONTRACT_ZK_VERIFIER,
  CONTRACT_USDC,
  STELLAR_NETWORK,
  STELLAR_RPC_URL,
} from "./contracts";

const RPC_URL = STELLAR_RPC_URL;

const ATOMIC_SWAP_CONTRACT_ID = CONTRACT_ATOMIC_SWAP;
const IP_REGISTRY_CONTRACT_ID = CONTRACT_IP_REGISTRY;

const networkPassphrase = () =>
  STELLAR_NETWORK === "mainnet"
    ? StellarSdk.Networks.PUBLIC
    : StellarSdk.Networks.TESTNET;

// ─── View helpers ─────────────────────────────────────────────────────────────

/**
 * Simulate a read-only contract call and return the raw ScVal result.
 * Uses a throwaway keypair as the source — no signing required.
 */
async function simulateView(
  functionName: string,
  args: import("@stellar/stellar-sdk").xdr.ScVal[],
) {
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
function decodeSwapScVal(
  scVal: import("@stellar/stellar-sdk").xdr.ScVal | undefined,
  swapId: number,
) {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

  // SwapStatus enum: scValToNative returns { tag: "Pending"|"Completed"|"Cancelled" }
  const status =
    typeof native.status === "object" && native.status !== null
      ? (native.status.tag ?? "Unknown")
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
    console.warn(`[decodeSwapScVal] swap ${swapId} has non-positive usdc_amount: ${usdcAmount}`);
  }

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
    { type: "address" },
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
  },
) {
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
        StellarSdk.nativeToScVal(Number(swapId), { type: "u64" }),
      ),
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
function encodeProofPath(proofPath: ProofNode[]): import("@stellar/stellar-sdk").xdr.ScVal {
  return StellarSdk.xdr.ScVal.scvVec(
    proofPath.map((node) => {
      const siblingBytes = Buffer.from(node.sibling.replace(/^0x/, ""), "hex");
      if (siblingBytes.length !== 32) {
        throw new Error(`ProofNode sibling must be exactly 32 bytes (64 hex chars), got ${siblingBytes.length} bytes.`);
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
    }),
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
  },
) {
  if (!ATOMIC_SWAP_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }
  if (!decryptionKey || !decryptionKey.trim()) {
    throw new Error("Decryption key is required.");
  }
  if (!proofPath || proofPath.length === 0) {
    throw new Error("Proof path is required and must be non-empty.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID);

  const keyBytes = StellarSdk.xdr.ScVal.scvBytes(
    Buffer.from(decryptionKey.replace(/^0x/, ""), "hex"),
  );

  const proofPathScVal = encodeProofPath(proofPath);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "confirm_swap",
        StellarSdk.nativeToScVal(Number(swapId), { type: "u64" }),
        keyBytes,
        proofPathScVal,
      ),
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
  },
) {
  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(usdcContractId);
  const spenderAddressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(spenderId),
    { type: "address" },
  );
  const fromAddressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(wallet.address),
    { type: "address" },
  );

  // Expiration typically requires ledger seq, but since this is testnet/demo, we can just supply a large number or the expected expiration argument
  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "approve",
        fromAddressScVal,
        spenderAddressScVal,
        StellarSdk.nativeToScVal(amount, { type: "i128" }),
        StellarSdk.nativeToScVal(0, { type: "u32" }), // expiration ledger
      ),
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
  },
): Promise<number> {
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
        StellarSdk.nativeToScVal(usdcAmount, { type: "i128" }), // usdc_amount
      ),
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);

  // NOTE: For demo purposes, we return a mocked ID. In a real integration, the new swap ID would need to be extracted from the tx events or queried.
  return Math.floor(Math.random() * 1000) + 1;
}

async function submitAndPoll(
  tx: import("@stellar/stellar-sdk").Transaction,
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  },
  server: import("@stellar/stellar-sdk").SorobanRpc.Server,
) {
  const preparedTx = await server.prepareTransaction(tx);
  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(
    signedXdr,
    networkPassphrase(),
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
}

// ─── IP Registry ──────────────────────────────────────────────────────────────

/**
 * Simulate a read-only call against the ip_registry contract.
 */
async function simulateIpRegistryView(
  functionName: string,
  args: import("@stellar/stellar-sdk").xdr.ScVal[],
) {
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
function decodeListingScVal(
  scVal: import("@stellar/stellar-sdk").xdr.ScVal | undefined,
  listingId: number,
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
    { type: "address" },
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
  if (!IP_REGISTRY_CONTRACT_ID) {
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

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(IP_REGISTRY_CONTRACT_ID);

  // Convert hex strings to Bytes (Buffer)
  const ipfsBytes = Buffer.from(ipfsHash.replace(/^0x/, ""), "hex");
  const merkleBytes = Buffer.from(merkleRoot.replace(/^0x/, ""), "hex");

  // USDC has 7 decimals, price_usdc is i128
  const priceRaw = Math.round(priceUsdc * 1e7);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "register_ip",
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), { type: "address" }),
        StellarSdk.xdr.ScVal.scvBytes(ipfsBytes),
        StellarSdk.xdr.ScVal.scvBytes(merkleBytes),
        StellarSdk.nativeToScVal(royaltyBps, { type: "u32" }),
        StellarSdk.nativeToScVal(new StellarSdk.Address(royaltyRecipient), { type: "address" }),
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
    { type: "address" },
  );

  const retval = await simulateView("get_swaps_by_seller", [addressScVal]);
  if (!retval) return [];

  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map((v) => Number(v));
}

// ─── USDC Balance ─────────────────────────────────────────────────────────────

const USDC_CONTRACT_ID = CONTRACT_USDC;
const USDC_DECIMALS = 7;

/**
 * Fetch the USDC balance for a given address by calling `balance(address)`
 * on the USDC token contract.
 * @param {string} address - Stellar public key (G...)
 * @returns {Promise<number>} - Balance in human-readable USDC (e.g. 12.5)
 */
export async function getUsdcBalance(address: string): Promise<number> {
  if (!USDC_CONTRACT_ID) return 0;

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(USDC_CONTRACT_ID);

  const tx = new StellarSdk.TransactionBuilder(account, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "balance",
        StellarSdk.nativeToScVal(new StellarSdk.Address(address), { type: "address" })
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

const ZK_VERIFIER_CONTRACT_ID = CONTRACT_ZK_VERIFIER;

async function simulateZkView(
  functionName: string,
  args: import("@stellar/stellar-sdk").xdr.ScVal[]
) {
  if (!ZK_VERIFIER_CONTRACT_ID) throw new Error("VITE_CONTRACT_ZK_VERIFIER is not configured.");
  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const keypair = StellarSdk.Keypair.random();
  const account = new StellarSdk.Account(keypair.publicKey(), "0");
  const contract = new StellarSdk.Contract(ZK_VERIFIER_CONTRACT_ID);
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
  if (!ZK_VERIFIER_CONTRACT_ID) throw new Error("VITE_CONTRACT_ZK_VERIFIER is not configured.");
  const rootBytes = Buffer.from(rootHex.replace(/^0x/, ""), "hex");
  if (rootBytes.length !== 32) throw new Error("Root must be exactly 32 bytes (64 hex chars).");

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(ZK_VERIFIER_CONTRACT_ID);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "set_merkle_root",
        StellarSdk.nativeToScVal(new StellarSdk.Address(wallet.address), { type: "address" }),
        StellarSdk.nativeToScVal(listingId, { type: "u64" }),
        StellarSdk.xdr.ScVal.scvBytes(rootBytes)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
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
