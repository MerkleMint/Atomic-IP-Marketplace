import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { WalletProvider } from "./context/WalletContext";
import { WalletConnectButton } from "./components/WalletConnectButton";
import { MySwapsDashboard } from "./components/MySwapsDashboard";
import { MyListingsDashboard } from "./components/MyListingsDashboard";

function App() {
  const walletRoot = document.getElementById("wallet-root");
  const dashboardRoot = document.getElementById("dashboard-root");
  const listingsRoot = document.getElementById("listings-dashboard-root");

  return (
    <WalletProvider>
      {walletRoot && createPortal(<WalletConnectButton />, walletRoot)}
      {dashboardRoot && createPortal(<MySwapsDashboard />, dashboardRoot)}
      {listingsRoot && createPortal(<MyListingsDashboard />, listingsRoot)}
    </WalletProvider>
  );
}

const appRoot = document.createElement("div");
appRoot.id = "react-app-root";
appRoot.style.display = "none";
document.body.appendChild(appRoot);

createRoot(appRoot).render(<App />);
