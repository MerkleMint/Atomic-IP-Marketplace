import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { NetworkProvider } from "./context/NetworkContext";
import { WalletProvider } from "./context/WalletContext";
import { NetworkSelector } from "./components/NetworkSelector";
import { WalletConnectButton } from "./components/WalletConnectButton";
import { MySwapsDashboard } from "./components/MySwapsDashboard";
import { MyListingsDashboard } from "./components/MyListingsDashboard";
import { ListingsPage } from "./components/ListingsPage";
import { SwapPage } from "./components/SwapPage";

function App() {
  const networkRoot = document.getElementById("network-root");
  const walletRoot = document.getElementById("wallet-root");
  const dashboardRoot = document.getElementById("dashboard-root");
  const listingsRoot = document.getElementById("listings-dashboard-root");

  return (
    <NetworkProvider>
      <WalletProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/" element={<ListingsPage />} />
            <Route path="/swap/:id" element={<SwapPage />} />
          </Routes>
        </BrowserRouter>

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
