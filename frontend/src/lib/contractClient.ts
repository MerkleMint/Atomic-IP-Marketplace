import * as StellarSdk from "@stellar/stellar-sdk";

const RPC_URL =
  import.meta.env.VITE_STELLAR_RPC_URL ||
  "https://soroban-testnet.stellar.org";

const ATOMIC_SWAP_CONTRACT_ID = import.meta.env.VITE_CONTRACT_ATOMIC_SWAP;
const IP_REGISTRY_CONTRACT_ID = import.meta.env.VITE_CONTRACT_IP_REGISTRY;
const USDC_CONTRACT_ID = import.meta.env.VITE_CONTRACT_USDC ?? "";
const USDC_DECIMALS = 7;
const ZK_VERIFIER_CONTRACT_ID = import.meta.env.VITE_CONTRACT_ZK_VERIFIER ?? "";

const networkPassphrase = () =>
  import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? StellarSdk.Networks.PUBLIC
    : StellarSdk.Networks.TESTNET;

// ─── Interfaces ──────────────────────────────────────────────────────────────

export interface ProofNode {
  sibling: string;
  is_left: boolean;
}

// ─── View helpers ─────────────────────────────────────────────────────────────

async function simulateView(functionName: string, args: any[]) {
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

function decodeSwapScVal(scVal: any, swapId: number) {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

  const status =
    typeof native.status === "object" && native.status !== null
      ? native.status.tag ?? "Unknown"
      : String(native.status ?? "Unknown");

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

export async function getSwapsByBuyer(buyerAddress: string) {
  const addressScVal = StellarSdk.nativeToScVal(
    new StellarSdk.Address(buyerAddress),
    { type: "address" }
  );

  const retval = await simulateView("get_swaps_by_buyer", [addressScVal]);
  if (!retval) return [];

  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map((v) => Number(v));
}

export async function getSwap(swapId: number) {
  const swapIdScVal = StellarSdk.nativeToScVal(swapId, { type: "u64" });
  const retval = await simulateView("get_swap", [swapIdScVal]);
  return decodeSwapScVal(retval, swapId);
}

export async function getLedgerTimestamp(): Promise<number> {
  return Math.floor(Date.now() / 1000);
}

// ─── Mutations ────────────────────────────────────────────────────────────────

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

async function submitAndPoll(
  tx: any,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> },
  server: any
): Promise<any> {
  const preparedTx = await server.prepareTransaction(tx);
  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(signedXdr, networkPassphrase());

  const sendResult = await server.sendTransaction(signedTx);
  if (sendResult.status === "ERROR") {
    throw new Error(`Transaction failed: ${sendResult.errorResult}`);
  }

  let txResponse = await server.getTransaction(sendResult.hash);
  while (txResponse.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1500));
    txResponse = await server.getTransaction(sendResult.hash);
  }

  if (txResponse.status !== "SUCCESS") {
    throw new Error(`Transaction did not succeed: ${txResponse.status}`);
  }

  const meta = StellarSdk.xdr.TransactionMeta.fromXDR(txResponse.resultMetaXdr!.toString(), "base64");
  
  // FIX: Added non-null assertion to sorobanMeta() to resolve TS2531
  return meta.v3().sorobanMeta()!.returnValue();
}

// ─── IP Registry ──────────────────────────────────────────────────────────────

async function simulateIpRegistryView(functionName: string, args: any[]) {
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

function decodeListingScVal(scVal: any, listingId: number) {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal);
  if (!native || typeof native !== "object") return null;

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

export async function getListing(listingId: number) {
  const retval = await simulateIpRegistryView("get_listing", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
  ]);

  if (!retval) return null;
  return decodeListingScVal(retval, listingId);
}

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

async function simulateZkView(functionName: string, args: any[]) {
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

export async function setMerkleRoot(
  listingId: number,
  rootHex: string,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<void> {
  if (!ZK_VERIFIER_CONTRACT_ID) throw new Error("VITE_CONTRACT_ZK_VERIFIER is not configured.");
  const rootBytes = Buffer.from(rootHex.replace(/^0x/, ""), "hex");

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
        new StellarSdk.Address(wallet.address).toScVal(),
        StellarSdk.nativeToScVal(listingId, { type: "u64" }),
        StellarSdk.xdr.ScVal.scvBytes(rootBytes)
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function verifyPartialProof(
  listingId: number,
  leafHex: string,
  path: ProofNode[] // Updated to use the exported interface
): Promise<boolean> {
  const leafBytes = Buffer.from(leafHex.replace(/^0x/, ""), "hex");

  const pathScVal = StellarSdk.xdr.ScVal.scvVec(
    path.map((node) => {
      const siblingBytes = Buffer.from(node.sibling.replace(/^0x/, ""), "hex");
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

  const retval = await simulateZkView("verify_partial_proof", [
    StellarSdk.nativeToScVal(listingId, { type: "u64" }),
    StellarSdk.xdr.ScVal.scvBytes(leafBytes),
    pathScVal,
  ]);

  if (!retval) return false;
  const native = StellarSdk.scValToNative(retval);
  return Boolean(native);
}

export async function approveUsdc(
  contractId: string,
  spender: string,
  amount: number,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
) {
  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(contractId);
  const rawAmount = BigInt(Math.floor(amount * Math.pow(10, USDC_DECIMALS)));

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "approve",
        new StellarSdk.Address(wallet.address).toScVal(),
        new StellarSdk.Address(spender).toScVal(),
        StellarSdk.nativeToScVal(rawAmount, { type: "i128" }),
        StellarSdk.nativeToScVal(Number(sourceAccount.sequenceNumber()) + 10000, { type: "u32" })
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function initiateSwap(
  listingId: number,
  usdcAmount: number,
  zkVerifier: string,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<number> {
  if (!ATOMIC_SWAP_CONTRACT_ID) throw new Error("Atomic Swap contract ID is not configured.");

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID);
  const rawAmount = BigInt(Math.floor(usdcAmount * Math.pow(10, USDC_DECIMALS)));

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "initiate_swap",
        StellarSdk.nativeToScVal(listingId, { type: "u64" }),
        new StellarSdk.Address(wallet.address).toScVal(),
        new StellarSdk.Address(USDC_CONTRACT_ID).toScVal(),
        StellarSdk.nativeToScVal(rawAmount, { type: "i128" }),
        new StellarSdk.Address(zkVerifier).toScVal()
      )
    )
    .setTimeout(30)
    .build();

  const result = await submitAndPoll(tx, wallet, server);
  return Number(StellarSdk.scValToNative(result));
}