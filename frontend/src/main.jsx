import React from "react";
import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { WalletProvider } from "./context/WalletContext";
import { WalletConnectButton } from "./components/WalletConnectButton";
import { MySwapsDashboard } from "./components/MySwapsDashboard";

/**
 * App root.
 *
 * A single WalletProvider wraps both UI surfaces so they share wallet state.
 * React Portals are used to render each piece into its own DOM node while
 * keeping them in the same React tree (and therefore the same context).
 */
function App() {
  const walletRoot = document.getElementById("wallet-root");
  const dashboardRoot = document.getElementById("dashboard-root");

  return (
    <WalletProvider>
      {walletRoot && createPortal(<WalletConnectButton />, walletRoot)}
      {dashboardRoot && createPortal(<MySwapsDashboard />, dashboardRoot)}
    </WalletProvider>
  );
}

// Mount the whole app into a tiny hidden div that we inject ourselves
const appRoot = document.createElement("div");
appRoot.id = "react-app-root";
appRoot.style.display = "none";
document.body.appendChild(appRoot);

createRoot(appRoot).render(<App />);
