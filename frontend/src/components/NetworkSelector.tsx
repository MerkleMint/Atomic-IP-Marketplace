import { useNetwork } from "../context/NetworkContext";
import type { Network } from "../context/NetworkContext";
import "./NetworkSelector.css";

export function NetworkSelector() {
  const { network, setNetwork } = useNetwork();

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    setNetwork(e.target.value as Network);
  };

  return (
    <div className="ns">
      <label className="ns__label" htmlFor="network-select">
        Network
      </label>
      <select
        id="network-select"
        className={`ns__select ${network === "mainnet" ? "ns__select--mainnet" : ""}`}
        value={network}
        onChange={handleChange}
        aria-label="Select network"
      >
        <option value="testnet">Testnet</option>
        <option value="mainnet">Mainnet</option>
      </select>
      {network === "mainnet" && (
        <span className="ns__warn" role="alert" aria-live="polite">
          ⚠️ You are on Mainnet. Transactions use real funds.
        </span>
      )}
    </div>
  );
}
