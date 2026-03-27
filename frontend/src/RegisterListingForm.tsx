import React, { useState } from "react";
import { Loader2, CheckCircle2, AlertCircle, Info } from "lucide-react";
import { registerIp } from "./stellar-service";

interface Props {
  wallet: { address: string; signTransaction: (xdr: string) => Promise<string> };
  onSuccess?: (id: number) => void;
}

const IPFS_CID_REGEX = /^(Qm[1-9A-HJ-NP-Za-km-z]{44,}|b[a-z2-7]{58,})$/;

export function RegisterListingForm({ wallet, onSuccess }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successId, setSuccessId] = useState<number | null>(null);

  const [formData, setFormData] = useState({
    ipfsHash: "",
    merkleRoot: "",
    priceUsdc: "0",
    royaltyBps: "0",
    royaltyRecipient: wallet.address,
  });

  const validate = () => {
    if (!formData.ipfsHash.trim()) return "IPFS Hash is required.";
    if (!IPFS_CID_REGEX.test(formData.ipfsHash.trim())) return "Invalid IPFS CID format.";
    if (!formData.merkleRoot.trim()) return "Merkle Root is required.";
    if (!/^[0-9a-fA-F]{64}$/.test(formData.merkleRoot.replace(/^0x/, ""))) {
      return "Merkle Root must be a 32-byte hex string (64 characters).";
    }
    if (Number(formData.royaltyBps) > 10000) return "Royalty cannot exceed 100% (10000 BPS).";
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessId(null);

    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);
    try {
      const listingId = await registerIp({
        ipfsHash: formData.ipfsHash.trim(),
        merkleRoot: formData.merkleRoot.trim(),
        priceUsdc: parseFloat(formData.priceUsdc),
        royaltyBps: parseInt(formData.royaltyBps),
        royaltyRecipient: formData.royaltyRecipient.trim(),
      }, wallet);

      setSuccessId(listingId);
      onSuccess?.(listingId);
      // Reset form on success
      setFormData({ ...formData, ipfsHash: "", merkleRoot: "" });
    } catch (err) {
      setError(err instanceof Error ? err.message : "An unknown error occurred");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-xl mx-auto p-6 bg-card border border-border rounded-2xl shadow-sm">
      <div className="mb-6">
        <h2 className="text-2xl font-bold flex items-center gap-2">
          Register New IP Asset
        </h2>
        <p className="text-muted-foreground text-sm">
          Mint your intellectual property on the Stellar ledger.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium mb-1.5">IPFS Hash (CID)</label>
          <input
            type="text"
            className="w-full px-4 py-2 rounded-lg bg-secondary/50 border border-border focus:ring-2 focus:ring-primary/50 outline-none transition-all"
            placeholder="Qm..."
            value={formData.ipfsHash}
            onChange={(e) => setFormData({ ...formData, ipfsHash: e.target.value })}
            disabled={loading}
          />
        </div>

        <div>
          <label className="block text-sm font-medium mb-1.5">Merkle Root (Hex)</label>
          <input
            type="text"
            className="w-full px-4 py-2 rounded-lg bg-secondary/50 border border-border focus:ring-2 focus:ring-primary/50 outline-none transition-all"
            placeholder="0x..."
            value={formData.merkleRoot}
            onChange={(e) => setFormData({ ...formData, merkleRoot: e.target.value })}
            disabled={loading}
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium mb-1.5">Price (USDC)</label>
            <input
              type="number"
              step="0.0000001"
              className="w-full px-4 py-2 rounded-lg bg-secondary/50 border border-border focus:ring-2 focus:ring-primary/50 outline-none transition-all"
              value={formData.priceUsdc}
              onChange={(e) => setFormData({ ...formData, priceUsdc: e.target.value })}
              disabled={loading}
            />
          </div>
          <div>
            <label className="block text-sm font-medium mb-1.5">Royalty (BPS)</label>
            <input
              type="number"
              className="w-full px-4 py-2 rounded-lg bg-secondary/50 border border-border focus:ring-2 focus:ring-primary/50 outline-none transition-all"
              placeholder="250 = 2.5%"
              value={formData.royaltyBps}
              onChange={(e) => setFormData({ ...formData, royaltyBps: e.target.value })}
              disabled={loading}
            />
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium mb-1.5">Royalty Recipient</label>
          <input
            type="text"
            className="w-full px-4 py-2 rounded-lg bg-secondary/50 border border-border focus:ring-2 focus:ring-primary/50 outline-none transition-all font-mono text-xs"
            value={formData.royaltyRecipient}
            onChange={(e) => setFormData({ ...formData, royaltyRecipient: e.target.value })}
            disabled={loading}
          />
        </div>

        {error && (
          <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm flex items-center gap-2 border border-destructive/20 animate-in fade-in slide-in-from-top-1">
            <AlertCircle className="w-4 h-4" />
            {error}
          </div>
        )}

        {successId && (
          <div className="p-4 rounded-lg bg-green-500/10 text-green-600 dark:text-green-400 text-sm border border-green-500/20 animate-in zoom-in-95">
            <div className="flex items-center gap-2 font-bold mb-1">
              <CheckCircle2 className="w-5 h-5" />
              Listing Created Successfully!
            </div>
            <p>Your IP asset has been registered with ID: <span className="font-mono bg-green-500/20 px-1 rounded">#{successId}</span></p>
          </div>
        )}

        {!wallet.address ? (
          <div className="p-3 rounded-lg bg-blue-500/10 text-blue-600 text-sm flex items-center gap-2 border border-blue-500/20">
            <Info className="w-4 h-4" />
            Please connect your wallet to register IP.
          </div>
        ) : (
          <button
            type="submit"
            disabled={loading}
            className="w-full py-3 bg-primary text-primary-foreground rounded-xl font-bold shadow-lg shadow-primary/20 hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2"
          >
            {loading ? (
              <>
                <Loader2 className="w-5 h-5 animate-spin" />
                Signing Transaction...
              </>
            ) : (
              "Register IP Asset"
            )}
          </button>
        )}
      </form>
    </div>
  );
}