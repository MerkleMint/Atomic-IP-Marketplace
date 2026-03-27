import { useEffect, useState, useCallback, useMemo } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import { ShoppingCart, Loader2, AlertTriangle, Search, Filter } from "lucide-react";
import { useDebounce } from "../lib/debounce";

interface Listing {
  id: number;
  ipfs_hash: string;
  owner: string;
  price: number;
  status: 'available' | 'pending' | 'sold';
}

const mockListings: Listing[] = [
  {
    id: 1,
    ipfs_hash: "QmXyZ12345abcdefghijkLMNOPQRSTUVWXYZabcdef",
    owner: "GABCDEFGHJKLMNPQRSTUVXYZ23456789ABCDEFGHJK",
    price: 120,
    status: 'available',
  },
  {
    id: 2,
    ipfs_hash: "QmZyX54321mnopqrstuVWXYZabcdef1234567890",
    owner: "GABCDE1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    price: 225,
    status: 'pending',
  },
  {
    id: 3,
    ipfs_hash: "QmLmnopQRStuvwxyZABCDEF1234567890ghijklmnop",
    owner: "G1234567890ABCDEFGHJKLMNPQRSTUVWXYZabcdef",
    price: 89,
    status: 'sold',
  },
  {
    id: 4,
    ipfs_hash: "QmNew45678newipfshashforlistingfour",
    owner: "GNEWOWNER1234567890ABCDEF",
    price: 150,
    status: 'available',
  },
  {
    id: 5,
    ipfs_hash: "QmFive99999fivehashhere",
    owner: "GABCDE9999999999ABCDEF",
    price: 300,
    status: 'available',
  },
];


function truncateHash(hash: string): string {
  if (!hash) return "";
  if (hash.length <= 14) return hash;
  return `${hash.slice(0, 6)}...${hash.slice(-6)}`;
}

function truncateAddress(address: string): string {
  if (!address) return "";
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-6)}`;
}

async function fetchListings(): Promise<Listing[]> {
  // TODO: replace with real indexer API call once available
  // e.g. const res = await fetch("/api/listings");
  // return await res.json();
  await new Promise((resolve) => setTimeout(resolve, 750));
  return mockListings;
}

export function ListingsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [allListings, setAllListings] = useState<Listing[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  const initialSearch = searchParams.get('search') || '';
  const initialStatus = searchParams.get('status') || 'all';
  const [searchQuery, setSearchQuery] = useState(initialSearch);
  const [statusFilter, setStatusFilter] = useState<'all' | 'available' | 'pending' | 'sold'>(initialStatus as any);
  const debouncedSearch = useDebounce(searchQuery, 300);

  // Filtered listings
  const filteredListings = useMemo(() => {
    return allListings.filter((listing) => {
      const searchLower = debouncedSearch.toLowerCase();
      const matchesSearch = debouncedSearch === '' || 
        listing.id.toString().includes(debouncedSearch) ||
        listing.owner.toLowerCase().includes(searchLower) ||
        listing.ipfs_hash.toLowerCase().includes(searchLower);
      const matchesStatus = statusFilter === 'all' || listing.status === statusFilter;
      return matchesSearch && matchesStatus;
    });
  }, [allListings, debouncedSearch, statusFilter]);

  // Update URL params when filters change
  useEffect(() => {
    const params = new URLSearchParams();
    if (searchQuery) params.set('search', searchQuery);
    if (statusFilter !== 'all') params.set('status', statusFilter);
    setSearchParams(params, { replace: true });
  }, [debouncedSearch, statusFilter, searchQuery, setSearchParams]);

  // Load listings
  useEffect(() => {
    let mounted = true;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const data = await fetchListings();
        if (mounted) {
          setAllListings(data);
        }
      } catch (err) {
        if (mounted) {
          setError("Unable to fetch listings at this time. Please try again.");
        }
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    load();

    return () => {
      mounted = false;
    };
  }, []);


  const content = () => {
    if (filteredListings.length === 0) {
      return (
        <div className="rounded-xl border border-slate-200 bg-slate-50 p-6 text-slate-700">
          <p>No listings match your filters.</p>
          <p className="text-sm text-slate-500">Try adjusting your search or status filter.</p>
        </div>
      );
    }

    return (
      <div>
        <p className="mb-4 text-sm text-slate-500">
          Showing {filteredListings.length} of {allListings.length} listings
        </p>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {filteredListings.map((listing: Listing) => (
            <article
              key={listing.id}
              className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm hover:shadow-md"
            >
              <div className="mb-2 flex items-center justify-between text-xs text-slate-500">
                <span>Listing #{listing.id}</span>
                <span>{listing.price} USDC</span>
              </div>

              <div className="mb-3">
                <p className="text-xs text-slate-400">IPFS Hash</p>
                <p className="truncate text-sm font-medium text-slate-800" title={listing.ipfs_hash}>
                  {truncateHash(listing.ipfs_hash)}
                </p>
              </div>

              <div className="mb-4">
                <p className="text-xs text-slate-400">Owner</p>
                <p className="truncate text-sm text-slate-700" title={listing.owner}>
                  {truncateAddress(listing.owner)}
                </p>
              </div>

              <button
                className="inline-flex w-full items-center justify-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-400"
                onClick={() => navigate(`/swap/${listing.id}`)}
              >
                <ShoppingCart size={16} />
                Buy Now
              </button>
            </article>
          ))}
        </div>
      </div>
    );
  };


  // Filters UI
  const filters = (
    <div className="mb-6 flex flex-col gap-3 lg:flex-row lg:items-end lg:gap-4">
      <div className="relative flex-1">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 h-4 w-4" />
        <input
          type="search"
          placeholder="Search by owner, listing ID, or IPFS hash..."
          className="w-full rounded-lg border border-slate-200 pl-10 pr-4 py-2 text-sm focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-400"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
      </div>
      <div className="flex items-center gap-2">
        <Filter className="text-slate-400 h-4 w-4 flex-shrink-0" />
        <select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as any)}
          className="rounded-lg border border-slate-200 px-3 py-2 text-sm focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-400"
        >
          <option value="all">All Statuses</option>
          <option value="available">Available</option>
          <option value="pending">Swap Pending</option>
          <option value="sold">Sold</option>
        </select>
      </div>
    </div>
  );

  return (
    <section className="mx-auto max-w-7xl p-4">
      <div className="mb-6 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">Marketplace Listings</h1>
          <p className="text-sm text-slate-500">Browse all IP listings and select one to swap.</p>
        </div>
        {loading && (
          <div className="inline-flex items-center gap-2 text-sm text-slate-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading
          </div>
        )}
      </div>

      {!loading && !error && filters}
      {content()}
    </section>
  );

}
