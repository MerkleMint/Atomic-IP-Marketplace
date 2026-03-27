import * as StellarSdk from "@stellar/stellar-sdk";

const RPC_URL = import.meta.env.VITE_STELLAR_RPC_URL || "https://soroban-testnet.stellar.org";
const IP_REGISTRY_CONTRACT_ID = import.meta.env.VITE_CONTRACT_IP_REGISTRY;
const USDC_DECIMALS = 7;

const networkPassphrase = () =>
  import.meta.env.VITE_STELLAR_NETWORK === "mainnet"
    ? StellarSdk.Networks.PUBLIC
    : StellarSdk.Networks.TESTNET;

export interface RegisterIpParams {
  ipfsHash: string;
  merkleRoot: string; // Hex string
  priceUsdc: number;
  royaltyBps: number;
  royaltyRecipient: string;
}

/**
 * Registers a new IP listing on the Soroban contract.
 * @returns The newly created listing_id (u64 as number)
 */
export async function registerIp(
  params: RegisterIpParams,
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> }
): Promise<number> {
  if (!IP_REGISTRY_CONTRACT_ID) {
    throw new Error("VITE_CONTRACT_IP_REGISTRY is not configured.");
  }

  const server = new StellarSdk.SorobanRpc.Server(RPC_URL);
  const sourceAccount = await server.getAccount(wallet.address);
  const contract = new StellarSdk.Contract(IP_REGISTRY_CONTRACT_ID);

  // Prepare arguments
  const ipfsHashBytes = StellarSdk.xdr.ScVal.scvBytes(Buffer.from(params.ipfsHash));
  const merkleRootBytes = StellarSdk.xdr.ScVal.scvBytes(
    Buffer.from(params.merkleRoot.replace(/^0x/, ""), "hex")
  );
  const priceRaw = BigInt(Math.floor(params.priceUsdc * Math.pow(10, USDC_DECIMALS)));

  const tx = new StellarSdk.TransactionBuilder(sourceAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: networkPassphrase(),
  })
    .addOperation(
      contract.call(
        "register_ip",
        new StellarSdk.Address(wallet.address).toScVal(),
        ipfsHashBytes,
        merkleRootBytes,
        StellarSdk.nativeToScVal(params.royaltyBps, { type: "u32" }),
        new StellarSdk.Address(params.royaltyRecipient).toScVal(),
        StellarSdk.nativeToScVal(priceRaw, { type: "i128" })
      )
    )
    .setTimeout(30)
    .build();

  // Simulate and prepare
  const preparedTx = await server.prepareTransaction(tx);
  
  // Sign with wallet (Freighter/xBull/etc)
  const signedXdr = await wallet.signTransaction(preparedTx.toXDR());
  const signedTx = StellarSdk.TransactionBuilder.fromXDR(signedXdr as string, networkPassphrase());

  // Submit
  const sendResult = await server.sendTransaction(signedTx);
  if (sendResult.status === "ERROR") {
    throw new Error(`Transaction submission failed: ${sendResult.errorResult}`);
  }

  // Poll for result
  let txResponse = await server.getTransaction(sendResult.hash);
  while (txResponse.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 2000));
    txResponse = await server.getTransaction(sendResult.hash);
  }

  if (txResponse.status !== "SUCCESS" || !txResponse.resultMetaXdr) {
    throw new Error(`Transaction failed with status: ${txResponse.status}`);
  }

  // Extract return value (listing_id)
  const transactionMeta = StellarSdk.xdr.TransactionMeta.fromXDR(txResponse.resultMetaXdr.toString(), "base64");
  const sorobanMeta = transactionMeta.v3()?.sorobanMeta();
  if (!sorobanMeta) throw new Error("Soroban metadata not found in transaction result");
  const returnValue = sorobanMeta.returnValue();
  
  return Number(StellarSdk.scValToNative(returnValue));
}