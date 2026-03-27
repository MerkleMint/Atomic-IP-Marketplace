import {
  StellarWalletsKit,
  WalletNetwork,
  allowAllModules,
  FREIGHTER_ID,
  ISupportedWallet,
} from '@creit.tech/stellar-wallets-kit';

export { FREIGHTER_ID };
export type { ISupportedWallet };

const network =
  import.meta.env.VITE_STELLAR_NETWORK === 'mainnet'
    ? WalletNetwork.PUBLIC
    : WalletNetwork.TESTNET;

export const kit = new StellarWalletsKit({
  network,
  selectedWalletId: FREIGHTER_ID,
  modules: allowAllModules(),
});

export interface Wallet {
  address: string;
  walletId: string;
  signTransaction: (xdr: string) => Promise<string>;
}

export async function connectWallet(walletId: string): Promise<Wallet> {
  kit.setWallet(walletId);
  const { address } = await kit.getAddress();
  return {
    address,
    walletId,
    signTransaction: async (xdr: string) => {
      const { signedTxXdr } = await kit.signTransaction(xdr, { address });
      return signedTxXdr;
    },
  };
}

export async function getAvailableWallets(): Promise<ISupportedWallet[]> {
  return kit.getSupportedWallets();
}
