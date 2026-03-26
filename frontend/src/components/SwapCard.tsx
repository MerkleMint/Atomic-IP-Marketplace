import React from "react";
import { CancelSwapButton } from "./CancelSwapButton";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import type { Swap } from "../lib/contractClient";
import type { ConnectedWallet } from "../lib/walletKit";
import "./SwapCard.css";

interface Props {
  swap: Swap;
  ledgerTimestamp: number;
  wallet: ConnectedWallet;
  onSwapUpdated: () => void;
}

export function SwapCard({ swap, ledgerTimestamp, wallet, onSwapUpdated }: Props) {
  const isBuyer = wallet.address === swap.buyer;
  const isSeller = wallet.address === swap.seller;

  return (
    <div className="swap-card">
      <div className="swap-card__info">
        <span className="swap-card__id">Swap #{swap.id}</span>
        <span className="swap-card__status" data-status={swap.status}>
          {swap.status}
        </span>
        <span className="swap-card__amount">{swap.usdc_amount} USDC</span>
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
        <ConfirmSwapForm swap={swap} wallet={wallet} onSuccess={onSwapUpdated} />
      )}
    </div>
  );
}
