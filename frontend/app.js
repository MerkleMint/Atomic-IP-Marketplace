const RPC_URL = "https://soroban-testnet.stellar.org/soroban/rpc";
const NETWORK_PASSPHRASE = "Test SDF Network ; September 2015";

// Config - TODO: Update with deployed IDs from deploy_testnet.sh
const CONTRACT_IDS = {
  atomic_swap: "CA3...", // AtomicSwap contract ID
  ip_registry: "CB3...", // IpRegistry
  usdc: "CZKZWVFYUWSRBCKXHDA7Y7VKOO7YS3EBD4L7AARDD7FHKGRTPL5NJVS", // Testnet USDC
  zk_verifier: "CC3...", // ZKVerifier stub
};

// Demo sellers for listings (register via ip_registry.register_ip)
const DEMO_SELLERS = [
  "GBUZBSCHIUURACQ7DDHAQ6U5P4CVTQVOV4CQJLNY3SXHG2JAO3DZQDSW",
  "GCSP6M2SS2P6JOZOPWFGEUVU7G6O74PO5G223282TJEUFVDLV5ZJ7KLA",
  "GDSTRSWT56XOHVVZMK5UUJ3XMXDPENL16D52Y5YVSE5EUAJ777TWC6AUA",
];

const form = document.getElementById("keyForm");
const result = document.getElementById("result");
const statusSpan = document.getElementById("status");
const keySection = document.getElementById("keySection");
const keyTextarea = document.getElementById("key");
const copyBtn = document.getElementById("copyBtn");
const ipfsSection = document.getElementById("ipfsDecrypt");
const decryptBtn = document.getElementById("decryptBtn");
const decryptResult = document.getElementById("decryptResult");

// Helper: base64 to bytes
function base64ToBytes(base64) {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}

// Stub IPFS decrypt demo (XOR cipher as placeholder)
function demoDecrypt(keyBytes, dataBytes) {
  const decrypted = new Uint8Array(dataBytes.length);
  for (let i = 0; i < dataBytes.length; i++) {
    decrypted[i] = dataBytes[i] ^ keyBytes[i % keyBytes.length];
  }
  return new TextDecoder().decode(decrypted);
}

async function fetchSwapData(contractId, swapId) {
  // Soroban RPC: simulateTransaction with invokeHostFunction for view calls
  // Build XDR for invoke { get_swap_status(u64) -> Option<u32> } and get_decryption_key(u64) -> Option<Bytes>
  // Simple: use string placeholders; in prod use @stellar/stellar-sdk ^12+ with xdr.fromXDR

  // For demo, assume status Completed if key present (combine calls)
  // RPC call for get_decryption_key (view function)

  const swapIdU64 = BigInt(swapId);

  // Build minimal ReadXdrRequest (simplified, actual needs full XDR serialization)
  // Note: Pure JS XDR hard; demo assumes SDK CDN or stub success for static demo
  // Load Stellar SDK
  const { Server } =
    await import("https://unpkg.com/@stellar/stellar-sdk@12.0.0/dist/stellar-sdk.min.js");
  const server = new Server(RPC_URL, { allowHttp: false });

  try {
    // Assume deployed contract has client AtomicSwapClient
    // Static demo: simulate key fetch
    const status = "Completed"; // Stub; real: invokeView(get_swap_status)
    const keyBase64 = "c3VwZXItc2VjcmV0LWtleQ=="; // base64 'super-secret-key'

    statusSpan.textContent = status;
    result.classList.remove("hidden");

    if (status === "Completed") {
      keySection.classList.remove("hidden");
      ipfsSection.classList.remove("hidden");
      const keyBytes = base64ToBytes(keyBase64);
      keyTextarea.value = `Base64: ${keyBase64}\\nBytes: [${Array.from(keyBytes).join(", ")}]`;
    }
  } catch (error) {
    statusSpan.textContent = `Error: ${error.message}`;
  }
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const submitBtn = form.querySelector('button[type="submit"]');
  const originalText = submitBtn.textContent;
  submitBtn.disabled = true;
  submitBtn.innerHTML =
    '<span class="btn-spinner" aria-hidden="true"></span> Fetching…';
  try {
    const contractId = document.getElementById("contractId").value;
    const swapId = parseInt(document.getElementById("swapId").value);
    await fetchSwapData(contractId, swapId);
  } finally {
    submitBtn.disabled = false;
    submitBtn.textContent = originalText;
  }
});

copyBtn.addEventListener("click", () => {
  keyTextarea.select();
  document.execCommand("copy");
  copyBtn.textContent = "Copied!";
  setTimeout(() => (copyBtn.textContent = "Copy Key"), 2000);
});

decryptBtn.addEventListener("click", () => {
  const cid = document.getElementById("cid").value;
  if (!cid) return;
  const keyB64 = keyTextarea.value.match(/Base64: ([^\\n]+)/)?.[1];
  if (!keyB64) return;
  const originalText = decryptBtn.textContent;
  decryptBtn.disabled = true;
  decryptBtn.innerHTML =
    '<span class="btn-spinner" aria-hidden="true"></span> Decrypting…';
  try {
    const keyBytes = base64ToBytes(keyB64);
    // Stub data from CID (demo bytes)
    const stubData = new TextEncoder().encode("demo encrypted IP content");
    const decrypted = demoDecrypt(keyBytes, stubData);
    decryptResult.textContent = `CID ${cid} decrypted: ${decrypted}`;
    decryptResult.style.color = "green";
  } finally {
    decryptBtn.disabled = false;
    decryptBtn.textContent = originalText;
  }
});

// === Initiate Swap Functionality ===
const listingsGrid = document.getElementById("listingsGrid");
const listingsError = document.getElementById("listingsError");
const initiateModal = document.getElementById("initiateModal");
const closeModal = document.querySelector(".close");
const listingDetails = document.getElementById("listingDetails");
const usdcAmountInput = document.getElementById("usdcAmount");
const approveBtn = document.getElementById("approveBtn");
const initiateBtn = document.getElementById("initiateBtn");
const txResult = document.getElementById("txResult");

let selectedListing = null;
let approvedAmount = 0;

// Load Stellar SDK
let StellarSDK;
async function loadSDK() {
  if (!StellarSDK) {
    const { Server, Contract, xdr, StrKey, Keypair } =
      await import("https://unpkg.com/@stellar/stellar-sdk@12.0.0/dist/stellar-sdk.min.js");
    StellarSDK = { Server, Contract, xdr, StrKey, Keypair };
  }
  return StellarSDK;
}

// Fetch listings from demo sellers
async function fetchListings() {
  listingsError.classList.add("hidden");
  listingsGrid.innerHTML =
    '<div class="listings-loading"><span class="btn-spinner btn-spinner--lg" aria-hidden="true"></span> Loading listings...</div>';
  document.querySelectorAll(".initiate-btn").forEach((btn) => {
    btn.disabled = true;
  });
  const { Server, xdr, Address } = await loadSDK();
  const server = new Server(RPC_URL, { allowHttp: false });

  try {
    const listings = [];
    for (const sellerStr of DEMO_SELLERS) {
      const seller = Address.fromString(sellerStr, "Account");
      // Simulate RPC call for list_by_owner(u64[])
      // Real: build invoke { list_by_owner(Address) -> Vec<u64> }
      // Demo: stub listings
      listings.push({
        id: listings.length + 1,
        seller: sellerStr.slice(0, 8) + "...",
        ipfs: `QmDemo${listings.length + 1}Hash`,
        merkle: "abc123def456...",
      });
    }
    renderListings(listings);
  } catch (error) {
    listingsError.textContent = `Error loading listings: ${error.message}`;
    listingsError.classList.remove("hidden");
  }
}

function renderListings(listings) {
  listingsGrid.innerHTML = listings
    .map(
      (listing) => `
     <div class="listing-card">
       <h3>Listing #${listing.id}</h3>
       <p><strong>Seller:</strong> ${listing.seller}</p>
       <p><strong>IPFS CID:</strong></p>
       <div class="listing-ipfs">${listing.ipfs}</div>
       <p><strong>Merkle Root:</strong> ${listing.merkle}</p>
       <button class="initiate-btn" onclick="openSwapModal(${JSON.stringify(listing)})">Initiate Swap</button>
     </div>
   `,
    )
    .join("");
}

// Delegate swap initiation to the React InitiateSwapModal via custom event
window.openSwapModal = function (listing) {
  window.dispatchEvent(
    new CustomEvent("open-initiate-swap", { detail: listing }),
  );
};

// Refresh listings when a swap is successfully initiated
window.addEventListener("swap-initiated", () => fetchListings());

// Init
document.addEventListener("DOMContentLoaded", () => {
  fetchListings();
});
