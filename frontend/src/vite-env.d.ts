/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_STELLAR_NETWORK: string;
  readonly VITE_STELLAR_RPC_URL: string;
  readonly VITE_CONTRACT_ATOMIC_SWAP: string;
  readonly VITE_CONTRACT_IP_REGISTRY: string;
  readonly VITE_CONTRACT_ZK_VERIFIER: string;
  readonly VITE_CONTRACT_USDC: string;
  readonly VITE_MAINNET_CONTRACT_ATOMIC_SWAP: string;
  readonly VITE_MAINNET_CONTRACT_IP_REGISTRY: string;
  readonly VITE_MAINNET_CONTRACT_ZK_VERIFIER: string;
  readonly VITE_IPFS_GATEWAY: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
