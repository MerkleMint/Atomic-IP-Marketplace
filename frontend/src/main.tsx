import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { WalletProvider } from "./context/WalletContext";
import { NetworkProvider } from "./context/NetworkContext";
import { WalletConnectButton } from "./components/WalletConnectButton";
import { NetworkSelector } from "./components/NetworkSelector";
import { MySwapsDashboard } from "./components/MySwapsDashboard";
// @ts-ignore
import { MyListingsDashboard } from "./components/MyListingsDashboard.tsx";

function App() {
  const walletRoot = document.getElementById("wallet-root");
  const networkRoot = document.getElementById("network-root");
  const dashboardRoot = document.getElementById("dashboard-root");
  const listingsRoot = document.getElementById("listings-dashboard-root");

  return ( 
    <NetworkProvider>
      <WalletProvider>
        {networkRoot && createPortal(<NetworkSelector />, networkRoot)}
        {walletRoot && createPortal(<WalletConnectButton />, walletRoot)}
        {dashboardRoot && createPortal(<MySwapsDashboard />, dashboardRoot)}
        {listingsRoot && createPortal(<MyListingsDashboard />, listingsRoot)}
      </WalletProvider>
    </NetworkProvider>
  );
}

const appRoot = document.createElement("div");
appRoot.id = "react-app-root";
appRoot.style.display = "none";
document.body.appendChild(appRoot);

createRoot(appRoot).render(<App />);
