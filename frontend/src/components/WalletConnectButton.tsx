import { useState, useEffect, useRef } from "react";
import { useWallet } from "../context/WalletContext";
import type { ISupportedWallet } from "../lib/walletKit";
import "./WalletConnectButton.css";

export function WalletConnectButton() {
  const { wallet, connecting, error, availableWallets, connect, disconnect } = useWallet();
  const [modalOpen, setModalOpen] = useState(false);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [connectError, setConnectError] = useState<string | null>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!modalOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setModalOpen(false); };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [modalOpen]);

  useEffect(() => {
    if (modalOpen) modalRef.current?.focus();
  }, [modalOpen]);

  const handleWalletSelect = async (walletId: string) => {
    setConnectError(null);
    setPendingId(walletId);
    try {
      await connect(walletId);
      setModalOpen(false);
    } catch (err) {
      setConnectError(err instanceof Error ? err.message : "Connection failed.");
    } finally {
      setPendingId(null);
    }
  };

  if (wallet) {
    const short = `${wallet.address.slice(0, 4)}…${wallet.address.slice(-4)}`;
    return (
      <div className="wck-connected">
        <span className="wck-address" title={wallet.address}>
          <span className="wck-dot" aria-hidden="true" />
          {wallet.walletId} · {short}
        </span>
        <button className="wck-btn wck-btn--disconnect" onClick={disconnect}>Disconnect</button>
      </div>
    );
  }

  // Show reconnection error when connecting is true and error is set (from auto-reconnect)
  const isReconnectFailed = connecting === false && error !== null && !wallet;

  return (
    <>
      <button
        className="wck-btn wck-btn--connect"
        onClick={() => { setConnectError(null); setModalOpen(true); }}
        disabled={connecting}
        aria-busy={connecting}
      >
        {connecting ? "Connecting…" : isReconnectFailed ? "Reconnection failed" : "Connect Wallet"}
      </button>

      {isReconnectFailed && (
        <p className="wck-btn--error" role="alert">{error}</p>
      )}

      {modalOpen && (
        <div
          className="wck-backdrop"
          role="dialog"
          aria-modal="true"
          aria-label="Select a wallet"
          onClick={(e) => { if (e.target === e.currentTarget) setModalOpen(false); }}
        >
          <div className="wck-modal" ref={modalRef} tabIndex={-1}>
            <div className="wck-modal__header">
              <h2 className="wck-modal__title">Connect Wallet</h2>
              <button className="wck-modal__close" onClick={() => setModalOpen(false)} aria-label="Close">×</button>
            </div>
            <ul className="wck-wallet-list" role="list">
              {availableWallets.map((w: ISupportedWallet) => {
                const isPending = pendingId === w.id;
                return (
                  <li key={w.id} className="wck-wallet-item">
                    <button
                      className="wck-wallet-btn"
                      onClick={() => handleWalletSelect(w.id)}
                      disabled={!!pendingId}
                      aria-busy={isPending}
                    >
                      <span className="wck-wallet-btn__name">{w.name}</span>
                      {isPending && <span className="wck-spinner" aria-label="Connecting…" />}
                    </button>
                  </li>
                );
              })}
            </ul>
            {(connectError || error) && (
              <p className="wck-modal__error" role="alert">{connectError || error}</p>
            )}
          </div>
        </div>
      )}
    </>
  );
}
