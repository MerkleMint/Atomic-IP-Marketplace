import { useState } from "react";
import { IpVersion } from "../lib/types";
import { USDC_DECIMALS } from "../lib/types";
import "./VersionHistoryBrowser.css";

interface IVersionHistoryBrowser {
  versions: IpVersion[];
  currentVersion: number;
}

function formatTimestamp(ts: number): string {
  if (!ts) return "—";
  return new Date(ts * 1000).toLocaleString();
}

export function VersionHistoryBrowser({
  versions,
  currentVersion,
}: IVersionHistoryBrowser) {
  const [expanded, setExpanded] = useState<number | null>(null);

  if (versions.length === 0) {
    return (
      <p className="vhb__empty">
        No version history. Use <em>Create Version</em> to start tracking
        changes.
      </p>
    );
  }

  const sorted = [...versions].sort(
    (a, b) => b.version_number - a.version_number
  );

  return (
    <div className="vhb">
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
    </div>
  );
}
