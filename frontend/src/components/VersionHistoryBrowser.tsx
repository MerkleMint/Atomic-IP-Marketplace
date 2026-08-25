import { useState, useEffect, useCallback } from "react";
import { IpVersion } from "../lib/types";
import { USDC_DECIMALS } from "../lib/types";
import { getVersionHistoryPage } from "../lib/contractClient";
import "./VersionHistoryBrowser.css";

const PAGE_SIZE = 10;

interface IVersionHistoryBrowser {
  listingId: number;
  currentVersion: number;
}

function formatTimestamp(ts: number): string {
  if (!ts) return "—";
  return new Date(ts * 1000).toLocaleString();
}

export function VersionHistoryBrowser({
  listingId,
  currentVersion,
}: IVersionHistoryBrowser) {
  const [versions, setVersions] = useState<IpVersion[]>([]);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<number | null>(null);

  const fetchFrom = useCallback(async (startOffset: number, replace: boolean) => {
    setLoading(true);
    setError(null);
    try {
      const page = await getVersionHistoryPage(listingId, startOffset, PAGE_SIZE);
      setVersions((prev) => (replace ? page : [...prev, ...page]));
      setOffset(startOffset + page.length);
      setHasMore(page.length === PAGE_SIZE);
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : "Failed to load version history."
      );
    } finally {
      setLoading(false);
    }
  }, [listingId]);

  useEffect(() => {
    setVersions([]);
    setOffset(0);
    setHasMore(true);
    setError(null);
    if (currentVersion > 0) {
      fetchFrom(0, true);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listingId, currentVersion]);

  const loadMore = () => fetchFrom(offset, false);

  if (currentVersion === 0 && versions.length === 0 && !loading && !error) {
    return (
      <p className="vhb__empty">
        No version history. Use <em>Create Version</em> to start tracking
        changes.
      </p>
    );
  }

  if (loading && versions.length === 0) {
    return <p className="vhb__empty">Loading version history…</p>;
  }

  if (error && versions.length === 0) {
    return <p className="vhb__error">{error}</p>;
  }

  const sorted = [...versions].sort(
    (a, b) => b.version_number - a.version_number
  );

  return (
    <div className="vhb">
      {error && <p className="vhb__error">{error}</p>}
      <ul className="vhb__list">
        {sorted.map((v) => {
          const isCurrent = v.version_number === currentVersion;
          const isOpen = expanded === v.version_number;
          return (
            <li key={v.version_number} className="vhb__item">
              <button
                className={`vhb__header${isCurrent ? " vhb__header--current" : ""}`}
                onClick={() =>
                  setExpanded(isOpen ? null : v.version_number)
                }
                aria-expanded={isOpen}
              >
                <span className="vhb__version-badge">v{v.version_number}</span>
                {isCurrent && (
                  <span className="vhb__current-tag">current</span>
                )}
                <span className="vhb__changelog-preview">
                  {v.changelog.length > 60
                    ? v.changelog.slice(0, 60) + "…"
                    : v.changelog}
                </span>
                <span className="vhb__chevron">{isOpen ? "▾" : "▸"}</span>
              </button>

              {isOpen && (
                <div className="vhb__detail">
                  <dl className="vhb__dl">
                    <dt>Changelog</dt>
                    <dd>{v.changelog}</dd>

                    <dt>IPFS Hash</dt>
                    <dd className="vhb__mono">
                      {v.ipfs_hash.slice(0, 20)}…
                    </dd>

                    <dt>Merkle Root</dt>
                    <dd className="vhb__mono">
                      {v.merkle_root.slice(0, 20)}…
                    </dd>

                    <dt>Price</dt>
                    <dd>
                      {(v.price_usdc / Math.pow(10, USDC_DECIMALS)).toFixed(2)}{" "}
                      USDC
                    </dd>

                    <dt>Royalty</dt>
                    <dd>{(v.royalty_bps / 100).toFixed(2)}%</dd>

                    <dt>Created</dt>
                    <dd>{formatTimestamp(v.timestamp)}</dd>

                    <dt>Created by</dt>
                    <dd className="vhb__mono">
                      {v.created_by.slice(0, 12)}…
                    </dd>
                  </dl>
                </div>
              )}
            </li>
          );
        })}
      </ul>

      {hasMore && (
        <button
          className="vhb__load-more"
          onClick={loadMore}
          disabled={loading}
        >
          {loading ? "Loading…" : "Load more"}
        </button>
      )}
    </div>
  );
}
