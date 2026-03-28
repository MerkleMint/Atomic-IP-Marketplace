import {
  StellarWalletsKit,
  WalletNetwork,
  allowAllModules,
  FREIGHTER_ID,
  ISupportedWallet,
} from '@creit.tech/stellar-wallets-kit';

export { FREIGHTER_ID, WalletNetwork };
export type { ISupportedWallet };

const initialNetwork =
  (() => {
    const saved = localStorage.getItem('selected_network');
    return saved === 'mainnet' ? WalletNetwork.PUBLIC : WalletNetwork.TESTNET;
  })();

export let kit = new StellarWalletsKit({
  network: initialNetwork,
  selectedWalletId: FREIGHTER_ID,
  modules: allowAllModules(),
});

/** Recreate the kit with a new network (called by NetworkContext on switch). */
export function reinitKit(network: WalletNetwork): void {
  kit = new StellarWalletsKit({
    network,
    selectedWalletId: FREIGHTER_ID,
    modules: allowAllModules(),
  });
}

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
      // Use kit at call time (may have been reinitialized)
      const { signedTxXdr } = await kit.signTransaction(xdr, { address });
      return signedTxXdr;
    },
  };
}

export async function getAvailableWallets(): Promise<ISupportedWallet[]> {
  return kit.getSupportedWallets();
}
