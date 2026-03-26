import * as StellarSdk from "@stellar/stellar-sdk";
import type { ConnectedWallet } from "./walletKit";

const RPC_URL =
  import.meta.env.VITE_STELLAR_RPC_URL ?? "https://soroban-testnet.stellar.org";

const ATOMIC_SWAP_CONTRACT_ID = import.meta.env.VITE_CONTRACT_ATOMIC_SWAP as string | undefined;

function networkPassphrase(): string {
  return import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? StellarSdk.Networks.PUBLIC
    : StellarSdk.Networks.TESTNET;
}

// ─── Types ────────────────────────────────────────────────────────────────────

export interface Swap {
  id: number;
  listing_id: number;
  buyer: string;
  seller: string;
  usdc_amount: number;
  created_at: number;
  expires_at: number;
  status: string;
  decryption_key: string | null;
}

// ─── View helpers ─────────────────────────────────────────────────────────────

async function simulateView(
  functionName: string,
  args: StellarSdk.xdr.ScVal[]
): Promise<StellarSdk.xdr.ScVal | undefined> {
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

  return (result as StellarSdk.SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
}

function decodeSwapScVal(scVal: StellarSdk.xdr.ScVal, swapId: number): Swap | null {
  if (!scVal || scVal.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(scVal) as Record<string, unknown>;
  if (!native || typeof native !== "object") return null;

  const status =
    typeof native.status === "object" && native.status !== null
      ? ((native.status as { tag?: string }).tag ?? "Unknown")
      : String(native.status ?? "Unknown");

  let decryptionKey: string | null = null;
  const rawKey = native.decryption_key;
  if (rawKey instanceof Uint8Array || Buffer.isBuffer(rawKey)) {
    decryptionKey = Buffer.from(rawKey as Uint8Array).toString("hex");
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

export async function getSwapsByBuyer(buyerAddress: string): Promise<number[]> {
  const addressScVal = StellarSdk.nativeToScVal(new StellarSdk.Address(buyerAddress), {
    type: "address",
  });

  const retval = await simulateView("get_swaps_by_buyer", [addressScVal]);
  if (!retval) return [];

  const arr = StellarSdk.scValToNative(retval);
  if (!Array.isArray(arr)) return [];
  return arr.map((v) => Number(v));
}

export async function getSwap(swapId: number): Promise<Swap | null> {
  if (!ATOMIC_SWAP_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);

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

  const swapScVal = response.entries[0].val.contractData().val();
  return decodeSwapScVal(swapScVal, swapId);
}

export async function getDecryptionKey(swapId: number): Promise<string | null> {
  const retval = await simulateView("get_decryption_key", [
    StellarSdk.nativeToScVal(swapId, { type: "u64" }),
  ]);
  if (!retval || retval.switch().name === "scvVoid") return null;

  const native = StellarSdk.scValToNative(retval);
  // Contract returns Option<Bytes> — native will be Uint8Array or null/undefined
  if (native instanceof Uint8Array || Buffer.isBuffer(native)) {
    return Buffer.from(native as Uint8Array).toString("hex");
  }
  return null;
}

export async function getLedgerTimestamp(): Promise<number> {
  return Math.floor(Date.now() / 1000);
}

// ─── Mutations ────────────────────────────────────────────────────────────────

export async function cancelSwap(swapId: number, wallet: ConnectedWallet): Promise<void> {
  if (!ATOMIC_SWAP_CONTRACT_ID) throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(ATOMIC_SWAP_CONTRACT_ID);

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call("cancel_swap", StellarSdk.nativeToScVal(swapId, { type: "u64" }))
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

export async function confirmSwap(
  swapId: number,
  decryptionKey: string,
  wallet: ConnectedWallet
): Promise<void> {
  if (!ATOMIC_SWAP_CONTRACT_ID) throw new Error("VITE_CONTRACT_ATOMIC_SWAP is not configured.");
  if (!decryptionKey.trim()) throw new Error("Decryption key is required.");

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
        StellarSdk.nativeToScVal(swapId, { type: "u64" }),
        keyBytes
      )
    )
    .setTimeout(30)
    .build();

  await submitAndPoll(tx, wallet, server);
}

// ─── Shared submit helper ─────────────────────────────────────────────────────

async function submitAndPoll(
  tx: StellarSdk.Transaction,
  wallet: ConnectedWallet,
  server: StellarSdk.SorobanRpc.Server
): Promise<void> {
  const preparedTx = await server.prepareTransaction(tx);
  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(signedXdr, networkPassphrase());

  const result = await server.sendTransaction(signedTx as StellarSdk.Transaction);
  if (result.status === "ERROR") {
    throw new Error(`Transaction failed: ${JSON.stringify(result.errorResult)}`);
  }

  let response = await server.getTransaction(result.hash);
  while (
    (response as { status: string }).status === "PENDING" ||
    (response as { status: string }).status === "NOT_FOUND"
  ) {
    await new Promise((r) => setTimeout(r, 1500));
    response = await server.getTransaction(result.hash);
  }

  if ((response as { status: string }).status !== "SUCCESS") {
    throw new Error(`Transaction did not succeed: ${(response as { status: string }).status}`);
  }
}
