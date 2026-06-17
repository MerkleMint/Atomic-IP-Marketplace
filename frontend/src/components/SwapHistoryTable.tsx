import { useState, useMemo } from "react";
import { Swap } from "../hooks/useMySwaps";
import { SwapStatusBadge } from "./SwapStatusBadge";
import "./SwapHistoryTable.css";

type SortField = "id" | "created_at" | "status" | "usdc_amount";
type SortOrder = "asc" | "desc";

interface SwapHistoryTableProps {
  swaps: Swap[];
  currentPage?: number;
  pageSize?: number;
}

const statusOrder: Record<string, number> = {
  Pending: 0,
  Disputed: 1,
  Completed: 2,
  Cancelled: 3,
};

export function SwapHistoryTable({
  swaps,
  currentPage = 1,
  pageSize = 10,
}: SwapHistoryTableProps) {
  const [sortField, setSortField] = useState<SortField>("created_at");
  const [sortOrder, setSortOrder] = useState<SortOrder>("desc");
  const [filter, setFilter] = useState<"all" | "pending" | "completed">("all");

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortOrder(sortOrder === "asc" ? "desc" : "asc");
    } else {
      setSortField(field);
      setSortOrder("asc");
    }
  };

  const filtered = useMemo(() => {
    return swaps.filter((swap) => {
      if (filter === "pending") return swap.status === "Pending";
      if (filter === "completed") return swap.status !== "Pending";
      return true;
    });
  }, [swaps, filter]);

  const sorted = useMemo(() => {
    const copy = [...filtered];
    copy.sort((a, b) => {
      let aVal: any = a[sortField];
      let bVal: any = b[sortField];

      // Special handling for status sorting
      if (sortField === "status") {
        aVal = statusOrder[aVal] ?? 999;
        bVal = statusOrder[bVal] ?? 999;
      }

      const cmp =
        aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
      return sortOrder === "asc" ? cmp : -cmp;
    });
    return copy;
  }, [filtered, sortField, sortOrder]);

  const startIdx = (currentPage - 1) * pageSize;
  const paginatedSwaps = sorted.slice(startIdx, startIdx + pageSize);
  const totalPages = Math.ceil(sorted.length / pageSize);

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const formatAmount = (amount: number) => {
    return (amount / 1e7).toFixed(2);
  };

  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) {
      return <span className="sort-icon" aria-hidden="true">⇅</span>;
    }
    return (
      <span className="sort-icon sort-icon--active" aria-hidden="true">
        {sortOrder === "asc" ? "↑" : "↓"}
      </span>
    );
  };

  return (
    <div className="swap-history-table">
      {/* Filter Controls */}
      <div className="swap-history-table__filters">
        <fieldset>
          <legend className="swap-history-table__legend">Status Filter</legend>
          <div className="swap-history-table__filter-group">
            {(
              [
                { value: "all", label: "All" },
                { value: "pending", label: "Active" },
                { value: "completed", label: "Resolved" },
              ] as const
            ).map(({ value, label }) => (
              <label key={value} className="swap-history-table__filter-label">
                <input
                  type="radio"
                  name="status-filter"
                  value={value}
                  checked={filter === value}
                  onChange={(e) =>
                    setFilter(e.currentTarget.value as typeof filter)
                  }
                  className="swap-history-table__filter-input"
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
        </fieldset>
      </div>

      {/* Table */}
      {paginatedSwaps.length > 0 ? (
        <>
          <div className="swap-history-table__container">
            <table className="swap-history-table__table" role="grid">
              <thead>
                <tr>
                  <th
                    className="swap-history-table__header"
                    onClick={() => handleSort("id")}
                    role="columnheader"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleSort("id");
                      }
                    }}
                  >
                    <span className="swap-history-table__header-text">
                      ID <SortIcon field="id" />
                    </span>
                  </th>
                  <th
                    className="swap-history-table__header"
                    onClick={() => handleSort("created_at")}
                    role="columnheader"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleSort("created_at");
                      }
                    }}
                  >
                    <span className="swap-history-table__header-text">
                      Date <SortIcon field="created_at" />
                    </span>
                  </th>
                  <th
                    className="swap-history-table__header"
                    onClick={() => handleSort("status")}
                    role="columnheader"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleSort("status");
                      }
                    }}
                  >
                    <span className="swap-history-table__header-text">
                      Status <SortIcon field="status" />
                    </span>
                  </th>
                  <th
                    className="swap-history-table__header"
                    onClick={() => handleSort("usdc_amount")}
                    role="columnheader"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleSort("usdc_amount");
                      }
                    }}
                  >
                    <span className="swap-history-table__header-text">
                      Amount <SortIcon field="usdc_amount" />
                    </span>
                  </th>
                  <th className="swap-history-table__header" role="columnheader">
                    Action
                  </th>
                </tr>
              </thead>
              <tbody>
                {paginatedSwaps.map((swap) => (
                  <tr key={swap.id} className="swap-history-table__row">
                    <td className="swap-history-table__cell">
                      <code className="swap-history-table__code">#{swap.id}</code>
                    </td>
                    <td className="swap-history-table__cell">
                      {formatDate(swap.created_at)}
                    </td>
                    <td className="swap-history-table__cell">
                      <SwapStatusBadge status={swap.status as any} />
                    </td>
                    <td className="swap-history-table__cell">
                      <span className="swap-history-table__amount">
                        {formatAmount(swap.usdc_amount)} USDC
                      </span>
                    </td>
                    <td className="swap-history-table__cell">
                      <button
                        className="swap-history-table__action-btn"
                        aria-label={`View details for swap ${swap.id}`}
                      >
                        View
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="swap-history-table__pagination">
              <button
                disabled={currentPage === 1}
                className="swap-history-table__pagination-btn"
                aria-label="Previous page"
              >
                ← Prev
              </button>
              <span className="swap-history-table__pagination-info">
                Page {currentPage} of {totalPages}
              </span>
              <button
                disabled={currentPage === totalPages}
                className="swap-history-table__pagination-btn"
                aria-label="Next page"
              >
                Next →
              </button>
            </div>
          )}
        </>
      ) : (
        <div className="swap-history-table__empty">
          <p>No transactions found.</p>
        </div>
      )}
    </div>
  );
}
