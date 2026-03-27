import { ConfirmSwapForm } from "./ConfirmSwapForm";
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

/**
 * ListingCard
 *
 * Displays a single IP listing owned by the connected seller, including
 * any pending swaps that require confirmation.
 *
 * Props:
 *   listing  - { id, ipfs_hash, price_usdc, pendingSwaps: Swap[] }
 *   wallet   - connected wallet { address, signTransaction }
 *   onUpdated - callback to refresh data after a swap action
 */
export function ListingCard({ listing, wallet, onUpdated }: IListingCard) {
  const ipfsUrl = listing.ipfs_hash
    ? `${IPFS_GATEWAY}/${listing.ipfs_hash}`
    : null;

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
