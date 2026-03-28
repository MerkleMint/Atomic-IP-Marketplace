import React, { useState, useEffect, useRef } from "react";
import { useWallet } from "../context/WalletContext";
import { isWalletAvailable, WALLET_IDS } from "../lib/walletKit";
import "./WalletConnectButton.css";

const WALLETS = [
  {
    id: WALLET_IDS.FREIGHTER,
    name: "Freighter",
    description: "Official SDF browser extension",
    installUrl: "https://freighter.app",
  },
  {
    id: WALLET_IDS.XBULL,
    name: "xBull",
    description: "Feature-rich Stellar wallet",
    installUrl: "https://xbull.app",
  },
  {
    id: WALLET_IDS.LOBSTR,
    name: "Lobstr",
    description: "Mobile + signer extension",
    installUrl: "https://lobstr.co/signer-extension",
  },
];

/**
 * WalletConnectButton
 *
 * Renders a connect/disconnect button. When clicked while disconnected,
 * opens a modal listing Freighter, xBull, and Lobstr for selection.
 */
export function WalletConnectButton() {
  const { wallet, connecting, error, connect, disconnect } = useWallet();
  const [modalOpen, setModalOpen] = useState(false);
  const [pendingId, setPendingId] = useState(null);
  const [connectError, setConnectError] = useState(null);
  const modalRef = useRef(null);

  // Close modal on Escape key
  useEffect(() => {
    if (!modalOpen) return;
    const onKey = (e) => { if (e.key === "Escape") setModalOpen(false); };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [modalOpen]);

  // Trap focus inside modal when open
  useEffect(() => {
    if (modalOpen) modalRef.current?.focus();
  }, [modalOpen]);

  const handleConnectClick = () => {
    setConnectError(null);
    setModalOpen(true);
  };

  const handleWalletSelect = async (walletId) => {
    setConnectError(null);
    setPendingId(walletId);
    try {
      await connect(walletId);
      setModalOpen(false);
    } catch (err) {
      setConnectError(err.message || "Connection failed.");
    } finally {
      setPendingId(null);
    }
  };

  const handleDisconnect = () => {
    disconnect();
  };

  const handleBackdropClick = (e) => {
    if (e.target === e.currentTarget) setModalOpen(false);
  };

  // ── Connected state ──────────────────────────────────────────────────────
  if (wallet) {
    const short = `${wallet.address.slice(0, 4)}…${wallet.address.slice(-4)}`;
    return (
      <div className="wck-connected">
        <span className="wck-address" title={wallet.address}>
          <span className="wck-dot" aria-hidden="true" />
          {wallet.walletId} · {short}
        </span>
        <button className="wck-btn wck-btn--disconnect" onClick={handleDisconnect}>
          Disconnect
        </button>
      </div>
    );
  }

  // ── Disconnected state ───────────────────────────────────────────────────
  return (
    <>
      <button
        className="wck-btn wck-btn--connect"
        onClick={handleConnectClick}
        disabled={connecting}
        aria-busy={connecting}
      >
        {connecting ? "Connecting…" : "Connect Wallet"}
      </button>

      {modalOpen && (
        <div
          className="wck-backdrop"
          role="dialog"
          aria-modal="true"
          aria-label="Select a wallet"
          onClick={handleBackdropClick}
        >
          <div className="wck-modal" ref={modalRef} tabIndex={-1}>
            <div className="wck-modal__header">
              <h2 className="wck-modal__title">Connect Wallet</h2>
              <button
                className="wck-modal__close"
                onClick={() => setModalOpen(false)}
                aria-label="Close wallet selector"
              >
                ×
              </button>
            </div>

            <ul className="wck-wallet-list" role="list">
              {WALLETS.map((w) => {
                const available = isWalletAvailable(w.id);
                const isPending = pendingId === w.id;
                return (
                  <li key={w.id} className="wck-wallet-item">
                    {available ? (
                      <button
                        className="wck-wallet-btn"
                        onClick={() => handleWalletSelect(w.id)}
                        disabled={!!pendingId}
                        aria-busy={isPending}
                      >
                        <span className="wck-wallet-btn__name">{w.name}</span>
                        <span className="wck-wallet-btn__desc">{w.description}</span>
                        {isPending && (
                          <span className="wck-spinner" aria-label="Connecting…" />
                        )}
                      </button>
                    ) : (
                      <a
                        className="wck-wallet-btn wck-wallet-btn--install"
                        href={w.installUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        <span className="wck-wallet-btn__name">{w.name}</span>
                        <span className="wck-wallet-btn__desc">Not detected — click to install</span>
                        <span className="wck-wallet-btn__badge">Install</span>
                      </a>
                    )}
                  </li>
                );
              })}
            </ul>

            {(connectError || error) && (
              <p className="wck-modal__error" role="alert">
                {connectError || error}
              </p>
            )}
          </div>
        </div>
      )}
    </>
  );
}
