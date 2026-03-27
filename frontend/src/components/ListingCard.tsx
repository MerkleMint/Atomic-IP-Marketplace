import { useState, useEffect } from "react";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import { SetMerkleRootForm } from "./SetMerkleRootForm";
import "./ListingCard.css";

const IPFS_GATEWAY =
  import.meta.env.VITE_IPFS_GATEWAY || "https://gateway.pinata.cloud/ipfs";

interface IListingCard {
  listing: {
    id: number;
    ipfs_hash: string;
    price_usdc: number;
    pendingSwaps: any[];
  };
  wallet: {
    walletId: string;
    address: string;
    signTransaction: (tx: any) => Promise<any>;
  };
  onUpdated: () => void;
}

interface IMeta {
  title: string;
  description: string;
  file_type: string;
}

/**
 * useIpfsMetadata
 * Fetches JSON metadata from IPFS for a given hash.
 */
function useIpfsMetadata(ipfsHash: string) {
  const [meta, setMeta] = useState<IMeta | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!ipfsHash) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    fetch(`${IPFS_GATEWAY}/${ipfsHash}`)
      .then((r) => {
        if (!r.ok) throw new Error(`IPFS fetch failed (${r.status})`);
        return r.json();
      })
      .then((data) => {
        if (!cancelled) setMeta(data as IMeta);
      })
      .catch((err) => {
        if (!cancelled) setError(err.message);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [ipfsHash]);

  return { meta, loading, error };
}

/**
 * ListingCard
 *
 * Displays a single IP listing owned by the connected seller, including
 * any pending swaps that require confirmation, IPFS metadata preview,
 * and a Merkle root management panel.
 *
 * Props:
 *   listing  - { id, ipfs_hash, price_usdc, pendingSwaps: Swap[] }
 *   wallet   - connected wallet { address, walletId, signTransaction }
 *   onUpdated - callback to refresh data after a swap action
 */
export function ListingCard({ listing, wallet, onUpdated }: IListingCard) {
  const ipfsUrl = listing.ipfs_hash
    ? `${IPFS_GATEWAY}/${listing.ipfs_hash}`
    : null;

  const {
    meta,
    loading: metaLoading,
    error: metaError,
  } = useIpfsMetadata(listing.ipfs_hash);
  const [showMerkle, setShowMerkle] = useState(false);

  return (
    <article className="lc" aria-label={`Listing #${listing.id}`}>
      <div className="lc__header">
        <span className="lc__id">Listing #{listing.id}</span>
        {listing.price_usdc > 0 && (
          <span className="lc__price">
            {listing.price_usdc / 1_000_000} USDC
          </span>
        )}
      </div>

      {/* IPFS metadata preview */}
      <div className="lc__ipfs-preview">
        {metaLoading && (
          <div className="lc__skeleton-block" aria-label="Loading metadata…">
            <div className="lc__skeleton-line lc__skeleton-line--title" />
            <div className="lc__skeleton-line" />
            <div className="lc__skeleton-line lc__skeleton-line--short" />
          </div>
        )}
        {!metaLoading && meta && (
          <div className="lc__meta-preview">
            {meta.title && <p className="lc__meta-title">{meta.title}</p>}
            {meta.description && (
              <p className="lc__meta-desc">{meta.description}</p>
            )}
            {meta.file_type && (
              <span className="lc__meta-badge">{meta.file_type}</span>
            )}
          </div>
        )}
        {!metaLoading && metaError && (
          <p className="lc__meta-fallback" role="status">
            Metadata unavailable
          </p>
        )}
      </div>

      <div className="lc__meta">
        <span className="lc__label">IPFS Hash</span>
        {ipfsUrl ? (
          <a
            className="lc__hash"
            href={ipfsUrl}
            target="_blank"
            rel="noopener noreferrer"
            title={listing.ipfs_hash}
          >
            {listing.ipfs_hash.slice(0, 20)}…
          </a>
        ) : (
          <span className="lc__hash lc__hash--empty">—</span>
        )}
      </div>

      {/* Merkle root panel */}
      <div className="lc__merkle">
        <button
          className="lc__merkle-toggle"
          onClick={() => setShowMerkle((v) => !v)}
          aria-expanded={showMerkle}
        >
          {showMerkle ? "▾" : "▸"} Merkle Root / ZK Proof
        </button>
        {showMerkle && (
          <SetMerkleRootForm listingId={listing.id} wallet={wallet} />
        )}
      </div>

      {listing.pendingSwaps.length === 0 ? (
        <p className="lc__no-swaps">No pending swaps</p>
      ) : (
        <div className="lc__swaps">
          <span className="lc__swaps-label">
            Pending swaps
            <span className="lc__badge">{listing.pendingSwaps.length}</span>
          </span>
          <ul className="lc__swaps-list">
            {listing.pendingSwaps.map((swap) => (
              <li key={swap.id} className="lc__swap-item">
                <ConfirmSwapForm
                  swap={swap}
                  wallet={wallet}
                  onSuccess={onUpdated}
                />
              </li>
            ))}
          </ul>
        </div>
      )}
    </article>
  );
}
