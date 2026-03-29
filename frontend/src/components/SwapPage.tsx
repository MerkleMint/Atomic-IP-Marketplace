import { useParams, useNavigate } from "react-router-dom";
import { useEffect, useState, useCallback } from "react";
import { getSwap, getLedgerTimestamp } from "../lib/contractClient";
import type { Swap } from "../hooks/useMySwaps";
import { useWallet } from "../context/WalletContext";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import { CancelSwapButton } from "./CancelSwapButton";
import "./SwapPage.css";

export function SwapPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { wallet } = useWallet();
  const [swap, setSwap] = useState<Swap | null>(null);
  const [ledgerTimestamp, setLedgerTimestamp] = useState<number>(() => Math.floor(Date.now() / 1000));
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSwap = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    setError(null);
    try {
      // getSwap expects a number. useParams returns strings.
      const s = await getSwap(Number(id));
      const ts = await getLedgerTimestamp();
      
      if (!s) {
        setError("Swap not found.");
      } else {
        setSwap(s);
        setLedgerTimestamp(ts);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load swap details.");
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    fetchSwap();
  }, [fetchSwap]);

  if (loading) {
    return (
      <section className="swap-page-loading">
        <div className="swap-page-spinner" aria-hidden="true" />
        <p>Loading swap details...</p>
      </section>
    );
  }

  if (error || !swap) {
    return (
      <section className="swap-page-error p-6">
        <div className="mx-auto max-w-xl text-center">
          <h1 className="text-2xl font-bold mb-4">Error</h1>
          <p className="mb-6 text-slate-600">{error || "Swap not found."}</p>
          <button onClick={() => navigate(-1)} className="btn-secondary">Go Back</button>
        </div>
      </section>
    );
  }

  const isBuyer = wallet?.address === swap.buyer;
  const isSeller = wallet?.address === swap.seller;

  return (
    <section className="swap-page p-6">
      <div className="mx-auto max-w-3xl">
        <header className="swap-page__header">
          <div className="flex items-center gap-4">
            <button onClick={() => navigate(-1)} className="swap-page__back-btn" aria-label="Go back">
              ←
            </button>
            <h1 className="text-3xl font-bold">Swap Details</h1>
          </div>
          <span className={`status-badge status-badge--${swap.status.toLowerCase()}`}>
            {swap.status}
          </span>
        </header>

        <div className="grid gap-6 md:grid-cols-2">
          <div className="swap-card">
            <h2 className="swap-card__title">Listing Information</h2>
            <div className="swap-card__content">
              <div className="swap-card__row">
                <span className="swap-card__label">Swap ID</span>
                <span className="swap-card__value font-mono">#{swap.id}</span>
              </div>
              <div className="swap-card__row">
                <span className="swap-card__label">Listing ID</span>
                <span className="swap-card__value font-mono">#{swap.listing_id}</span>
              </div>
              <div className="swap-card__row">
                <span className="swap-card__label">USD Amount</span>
                <span className="swap-card__value font-semibold">{(swap.usdc_amount / 1e7).toFixed(2)} USDC</span>
              </div>
              <div className="swap-card__row">
                <span className="swap-card__label">Created</span>
                <span className="swap-card__value">{new Date(swap.created_at * 1000).toLocaleString()}</span>
              </div>
            </div>
          </div>

          <div className="swap-card">
            <h2 className="swap-card__title">Parties</h2>
            <div className="swap-card__content">
              <div className="party-info">
                <span className="party-info__role">Buyer</span>
                <span className="party-info__address font-mono" title={swap.buyer}>{swap.buyer}</span>
                {isBuyer && <span className="party-info__tag">You</span>}
              </div>
              <div className="party-info">
                <span className="party-info__role">Seller</span>
                <span className="party-info__address font-mono" title={swap.seller}>{swap.seller}</span>
                {isSeller && <span className="party-info__tag">You</span>}
              </div>
            </div>
          </div>
        </div>

        <div className="mt-8">
           {swap.status === "Pending" && (
             <div className="swap-actions">
               <h2 className="swap-actions__title">Actions Required</h2>

               {isSeller && (
                 <div className="swap-actions__form">
                   <p className="swap-actions__description">
                     To complete this swap and receive your USDC, you must provide the decryption key and Merkle proof path for the IP listing.
                   </p>
                   {wallet && (
                     <ConfirmSwapForm
                       swap={swap}
                       wallet={wallet}
                       onSuccess={fetchSwap}
                     />
                   )}
                 </div>
               )}

               {isBuyer && (
                 <div className="swap-actions__cancel">
                   <p className="swap-actions__description">
                     The swap is locked. If the seller does not complete the transaction before the expiry time, you can cancel and reclaim your USDC.
                   </p>
                   {wallet && (
                     <CancelSwapButton
                       swap={swap}
                       ledgerTimestamp={ledgerTimestamp}
                       wallet={wallet}
                       onSuccess={fetchSwap}
                     />
                   )}
                 </div>
               )}

               {!isBuyer && !isSeller && (
                 <div className="swap-actions__other">
                   <p className="italic text-slate-500">
                     You are not a participant in this swap. Only the buyer or seller can interact with it.
                   </p>
                 </div>
               )}
             </div>
           )}

           {swap.status === "Completed" && (
             <div className="swap-status-box swap-status-box--completed">
                <h2 className="swap-status-box__title">Swap Completed</h2>
                <p className="swap-status-box__desc">The transaction has been successfully settled. Atomic assets have been transferred.</p>
                {swap.decryption_key && (
                  <div className="swap-key-box">
                    <span className="swap-key-box__label">Decryption Key</span>
                    <code className="swap-key-box__value">{swap.decryption_key}</code>
                  </div>
                )}
             </div>
           )}

           {swap.status === "Cancelled" && (
              <div className="swap-status-box swap-status-box--cancelled">
                <h2 className="swap-status-box__title">Swap Cancelled</h2>
                <p className="swap-status-box__desc">This swap was cancelled and the buyer's funds have been returned.</p>
              </div>
           )}
        </div>
      </div>
    </section>
  );
}
