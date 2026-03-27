
import { CancelSwapButton } from "./CancelSwapButton";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import "./SwapCard.css";

interface Props {
  swap: Swap;
  ledgerTimestamp: number;
  wallet: Wallet;
  onSwapUpdated: () => void;
}

export function SwapCard({ swap, ledgerTimestamp, wallet, onSwapUpdated }: Props) {
  const isBuyer = wallet.address === swap.buyer;
  const isSeller = wallet.address === swap.seller;

  return (
    <div className="swap-card">
      <div className="swap-card__info">
        <span className="swap-card__id">Swap #{swap.id}</span>
        <span className="swap-card__status" data-status={swap.status}>{swap.status}</span>
        <span className="swap-card__amount">{swap.usdc_amount} USDC</span>
      </div>
      {isBuyer && (
        <CancelSwapButton swap={swap} ledgerTimestamp={ledgerTimestamp} wallet={wallet} onSuccess={onSwapUpdated} />
      )}
      {isSeller && (
        <ConfirmSwapForm swap={swap} wallet={wallet} onSuccess={onSwapUpdated} />
      )}
    </div>
  );
}
