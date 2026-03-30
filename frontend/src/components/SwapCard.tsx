import { CancelSwapButton } from "./CancelSwapButton";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import "./SwapCard.css";
import { CopyButton } from "./CopyButton";

const USDC_DECIMALS = 7;

interface Props {
  swap: Swap;
  ledgerTimestamp: number;
  wallet: Wallet;
  onSwapUpdated: () => void;
}

export function SwapCard({
  swap,
  ledgerTimestamp,
  wallet,
  onSwapUpdated,
}: Props) {
  const isBuyer = wallet.address === swap.buyer;
  const isSeller = wallet.address === swap.seller;

  return (
    <div className="swap-card">
      <div className="swap-card__info">
        <div className="swap-card__id flex items-center gap-2">
          <span>Swap #{swap.id}</span>
          <CopyButton text={swap.id.toString()} />
        </div>
        <span className="swap-card__status" data-status={swap.status}>
          {swap.status}
        </span>
        <span className="swap-card__amount">{(swap.usdc_amount / Math.pow(10, USDC_DECIMALS)).toFixed(2)} USDC</span>
      </div>
      {isBuyer && (
        <CancelSwapButton
          swap={swap}
          ledgerTimestamp={ledgerTimestamp}
          wallet={wallet}
          onSuccess={onSwapUpdated}
        />
      )}
      {isSeller && (
        <ConfirmSwapForm
          swap={swap}
          wallet={wallet}
          onSuccess={onSwapUpdated}
        />
      )}
    </div>
  );
}
