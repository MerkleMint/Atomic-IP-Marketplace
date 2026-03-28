## Issue #1: `atomic_swap` does not enforce ZK proof verification before confirming swap

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 2 hours

**Description:**
`confirm_swap` accepts a `decryption_key` and stores it without calling `zk_verifier.verify_partial_proof`. A seller can confirm any swap and collect payment without ever proving ownership of the IP asset.

**Tasks:**
- Call `zk_verifier.verify_partial_proof` inside `confirm_swap` before storing the key
- Reject with `ContractError::ProofInvalid` if verification returns `false`
- Add a test that passes an invalid proof and expects rejection

---

## Issue #2: `atomic_swap` `cancel_swap` has no time-lock enforcement

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1.5 hours

**Description:**
`cancel_swap` allows the buyer to cancel immediately after initiating. The `cancel_delay_secs` field exists in `Config` but is never checked. Buyers could cancel after receiving the decryption key.

**Tasks:**
- Read `env.ledger().timestamp()` in `cancel_swap` and compare against `swap.created_at + config.cancel_delay_secs`
- Return `ContractError::CancelTooEarly` if the delay has not elapsed
- Add tests for cancel before and after the delay window

---

## Issue #3: `atomic_swap` emits no events on swap state changes

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
None of the swap lifecycle functions (`initiate_swap`, `confirm_swap`, `cancel_swap`) emit on-chain events. Off-chain indexers and the frontend cannot react to state changes without polling.

**Tasks:**
- Emit `SwapInitiated { swap_id, buyer, seller, listing_id, usdc_amount }` in `initiate_swap`
- Emit `SwapConfirmed { swap_id }` in `confirm_swap`
- Emit `SwapCancelled { swap_id }` in `cancel_swap`
- Add tests asserting each event is emitted with correct data

---

## Issue #4: `ip_registry` TTL is hardcoded and not configurable

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`ip_registry` uses a hardcoded TTL constant for listings. There is no way to adjust the TTL without redeploying the contract, making it inflexible for different network conditions.

**Tasks:**
- Move TTL values to a `Config` struct stored at initialization
- Add an admin `update_ttl` function
- Add tests for TTL configuration and update

---

## Issue #5: `zk_verifier` Merkle proof uses a trivial XOR hash instead of a real hash function

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 3 hours

**Description:**
`verify_partial_proof` combines proof nodes using XOR (`^`), which is not a cryptographically secure hash function. Any two leaves with the same XOR result would produce the same root, making proofs trivially forgeable.

**Tasks:**
- Replace XOR with SHA-256 (via `soroban_sdk::crypto::sha256`) for node combination
- Update all existing test snapshots to use the new hash
- Document the proof format in a comment

---

## Issue #6: No `pause` / `unpause` function in `ip_registry`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`atomic_swap` has pause/unpause but `ip_registry` does not. In an emergency, the admin cannot stop new IP registrations without pausing the swap contract too.

**Tasks:**
- Add `pause(env)` and `unpause(env)` to `IpRegistry` gated by admin auth
- Block `register_ip` when paused
- Add tests matching the pattern in `atomic_swap`

---

## Issue #7: `atomic_swap` does not validate `usdc_amount > 0` on initiation

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`initiate_swap` does not check that `usdc_amount` is greater than zero. A buyer could initiate a swap for 0 USDC, and the seller would receive nothing after confirming.

**Tasks:**
- Add a guard `if usdc_amount <= 0 { panic_with_error!(env, ContractError::InvalidAmount) }`
- Add a test that passes `usdc_amount = 0` and expects the error

---

## Issue #8: `ip_registry` does not validate `price` field is positive

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`register_ip` accepts any `i128` price including zero and negative values. A listing with a zero or negative price would allow buyers to initiate swaps for free.

**Tasks:**
- Add `if price <= 0 { panic_with_error!(env, ContractError::InvalidPrice) }` in `register_ip`
- Add tests for zero and negative price inputs

---

## Issue #9: `Cargo.toml` workspace does not pin `soroban-sdk` version

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
The workspace `Cargo.toml` specifies `soroban-sdk` with a caret version (`^21`). A breaking minor or patch release could silently change contract behavior between builds.

**Tasks:**
- Pin `soroban-sdk` to an exact version (e.g. `= "21.7.6"`) in the workspace `Cargo.toml`
- Run `cargo update` and commit the updated `Cargo.lock`
- Document the pinned version in `README.md`

---

## Issue #10: `README.md` has no local development setup instructions

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The `README.md` lacks step-by-step instructions for setting up a local development environment. New contributors cannot build or test the project without prior Soroban knowledge.

**Tasks:**
- Add a "Getting Started" section with prerequisites (Rust, `stellar-cli`, Node.js)
- Document `./scripts/build.sh` and `./scripts/test.sh` usage
- Add a local testnet setup guide using `stellar network` commands
- Include a `.env.example` walkthrough

---

## Issue #11: `ip_registry` has no `update_listing` function

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 2 hours

**Description:**
Once an IP listing is registered, there is no way for the owner to update the `ipfs_hash` or `merkle_root`. Sellers cannot correct mistakes or publish new versions of their IP asset.

**Tasks:**
- Add `update_listing(env, owner, listing_id, ipfs_hash, merkle_root)` to `IpRegistry`
- Require auth from `owner` and verify `listing.owner == owner`
- Validate inputs are non-empty
- Add tests for successful update, unauthorized update, and non-existent listing

---

## Issue #12: `ip_registry` has no `deregister_listing` function

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1.5 hours

**Description:**
There is no way for an owner to remove a listing from the registry. This prevents sellers from taking down IP assets that are no longer for sale.

**Tasks:**
- Add `deregister_listing(env, owner, listing_id)` to `IpRegistry`
- Require auth and ownership check
- Remove from persistent storage and owner index
- Add tests for successful deregistration and unauthorized attempt

---

## Issue #13: `zk_verifier` `verify_partial_proof` panics when root is missing instead of returning `false`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`verify_partial_proof` calls `.expect("root not found")` when no Merkle root is stored for a listing. This causes a hard panic instead of returning `false` or a structured error, which breaks callers that expect a boolean result.

**Tasks:**
- Change the `expect` to return `false` (or `Err`) when root is missing
- Add a test that calls `verify_partial_proof` on a listing with no stored root and asserts `false`

---

## Issue #14: `zk_verifier` `set_merkle_root` uses raw `assert!` for unauthorized overwrite

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`set_merkle_root` uses `assert!(existing_owner == owner, "unauthorized: caller is not the listing owner")` instead of a structured `ContractError`. This is inconsistent with `ip_registry` which uses `panic_with_error!`.

**Tasks:**
- Add `#[contracterror]` enum to `zk_verifier` with `Unauthorized` variant
- Replace the `assert!` with `panic_with_error!`
- Update the test to match on the error variant

---

## Issue #15: No `transfer_listing_ownership` function in `ip_registry`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 2 hours

**Description:**
There is no mechanism to transfer ownership of a listing to another address. This is a fundamental requirement for a marketplace where IP assets may be sold or transferred.

**Tasks:**
- Add `transfer_ownership(env, current_owner, listing_id, new_owner)` to `IpRegistry`
- Require auth from `current_owner`, update `listing.owner`, update `OwnerIndex` for both addresses
- Add tests for successful transfer, unauthorized transfer, and index consistency

---

## Issue #16: `atomic_swap` `Counter` is stored in instance storage but swaps are in persistent storage

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1.5 hours

**Description:**
The swap `Counter` is stored in instance storage while individual `Swap` entries are in persistent storage. If instance storage expires before persistent storage, the counter resets to 0 and new swaps will overwrite existing ones with the same ID.

**Tasks:**
- Move `Counter` to persistent storage
- Add TTL extension for the counter key on every write
- Add a regression test that verifies IDs are unique across many swaps

---

## Issue #17: `ip_registry` `Counter` has the same instance/persistent storage mismatch

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
Same issue as #16 — `IpRegistry` stores its `Counter` in instance storage while listings are in persistent storage. Counter reset would cause listing ID collisions.

**Tasks:**
- Move `Counter` to persistent storage with TTL extension
- Add a test verifying listing IDs remain unique after many registrations

---

## Issue #18: No admin `update_config` function in `atomic_swap`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The `Config` (fee_bps, fee_recipient, cancel_delay_secs) is set once at initialization and cannot be changed. The admin has no way to adjust fees or the fee recipient without redeploying.

**Tasks:**
- Add `update_config(env, fee_bps, fee_recipient, cancel_delay_secs)` gated by admin auth
- Add tests for successful update and unauthorized attempt

---

## Issue #19: No admin transfer function in `atomic_swap`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 1 hour

**Description:**
The admin address is set at initialization and cannot be changed. If the admin key is compromised or needs rotation, there is no recovery path.

**Tasks:**
- Add `transfer_admin(env, new_admin)` requiring current admin auth
- Emit an `AdminTransferred` event
- Add tests for successful transfer and unauthorized attempt

---

## Issue #20: `get_listing` in `ip_registry` does not extend TTL on read

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
Reading a listing via `get_listing` does not extend its TTL. Active listings that are frequently read but not written will eventually expire from persistent storage.

**Tasks:**
- Call `extend_ttl` on the listing key inside `get_listing` when the entry exists
- Add a test that reads a listing near TTL expiry and verifies it persists

---

## Issue #21: No pagination support for `get_swaps_by_buyer`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 2 hours

**Description:**
`get_swaps_by_buyer` returns the entire `Vec<u64>` for a buyer. For active buyers with many swaps, this can exceed Soroban's return size limits and cause the call to fail.

**Tasks:**
- Add `get_swaps_by_buyer_page(env, buyer, offset, limit)` with bounds checking
- Keep the existing function for backward compatibility
- Add tests for pagination edge cases (empty, partial, full page)

---

## Issue #22: No pagination support for `list_by_owner` in `ip_registry`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1.5 hours

**Description:**
Same issue as #21 — `list_by_owner` returns all listing IDs at once, which can exceed size limits for prolific IP owners.

**Tasks:**
- Add `list_by_owner_page(env, owner, offset, limit)`
- Add tests for pagination correctness

---

## Issue #23: `deploy_testnet.sh` does not verify contracts after deployment

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`deploy_testnet.sh` deploys all three contracts but does not call any function to verify they are live and callable after deployment.

**Tasks:**
- Add a smoke-test invocation of `get_merkle_root` on `zk_verifier` post-deploy
- Add a smoke-test invocation of `get_listing` on `ip_registry` post-deploy
- Log contract addresses clearly in the output

---

## Issue #24: Missing `LICENSE` file

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
The `README.md` notes "TBD (add LICENSE file if needed)" but no `LICENSE` file exists. Open-source projects without a license are legally ambiguous.

**Tasks:**
- Add an `Apache-2.0` or `MIT` `LICENSE` file
- Update `README.md` to reflect the chosen license

---

## Issue #25: `build.sh` does not optimize WASM output size

**Labels:** `enhancement`

**Body:**

**Category:** DevOps - Enhancement

**Priority:** Low

**Estimated Time:** 1 hour

**Description:**
`build.sh` runs a plain `cargo build --release` without WASM optimization. Soroban contracts should be stripped and optimized with `stellar contract optimize` to reduce deployment costs.

**Tasks:**
- Add `stellar contract optimize` step for each WASM artifact after build
- Update `build.sh` to fail if optimization step fails
- Document the optimized output paths

---

## Issue #26: UI/UX — No loading states on contract calls

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** High

**Estimated Time:** 2 hours

**Description:**
Contract interactions can take several seconds to confirm on Stellar. Without loading indicators, users have no feedback and may click buttons multiple times, causing duplicate transactions.

**Tasks:**
- Disable all action buttons while a transaction is in-flight
- Show a spinner or skeleton inside the button during loading
- Re-enable the button and show result once the transaction settles

---

## Issue #27: UI/UX — Wallet connection state is not persisted across page refreshes

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
Users are logged out every time they refresh the page. The wallet connection state should be restored automatically if the user previously connected Freighter.

**Tasks:**
- On app load, check `freighter-api.isConnected()` and restore the session silently
- Show a "Reconnecting..." indicator while checking
- Only show the "Connect Wallet" button if the check returns false

---

## Issue #28: UI/UX — No empty state for listings page

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
When there are no IP listings, the listings page shows a blank area with no guidance. New users do not know what to do next.

**Tasks:**
- Add an empty state illustration and a "No listings yet" message
- Include a "Register your IP" call-to-action button that links to the registration form
- Show the empty state only after the data fetch completes (not during loading)

---

## Issue #29: UI/UX — Error messages show raw contract error codes

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** High

**Estimated Time:** 1.5 hours

**Description:**
When a contract call fails, the raw Soroban error code (e.g. `Error(Contract, #3)`) is shown directly to the user. This is not meaningful to non-developers.

**Tasks:**
- Create an error code mapping from contract error variants to human-readable strings
- Display the mapped message in the error toast or inline form error
- Fall back to a generic "Something went wrong" message for unmapped codes

---

## Issue #30: UI/UX — No confirmation dialog before irreversible actions

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
Actions like "Initiate Swap" and "Cancel Swap" are irreversible once submitted. There is no confirmation step, so users can accidentally trigger them.

**Tasks:**
- Add a modal confirmation dialog for `initiate_swap` showing the USDC amount and listing details
- Add a confirmation dialog for `cancel_swap` warning that funds will be returned
- Allow users to dismiss the dialog without proceeding

---

## Issue #31: UI/UX — Listing cards have no visual status indicator

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
Listing cards on the marketplace page do not show whether a listing is available, has a pending swap, or has been sold. Users cannot tell at a glance which listings they can act on.

**Tasks:**
- Add a status badge (Available / Swap Pending / Sold) to each listing card
- Use distinct colors for each status (green / yellow / grey)
- Disable the "Buy" button on listings that are not available

---

## Issue #32: UI/UX — No copy-to-clipboard for contract addresses and listing IDs

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
Contract addresses and listing IDs are displayed as plain text. Users must manually select and copy them, which is error-prone on mobile.

**Tasks:**
- Add a copy icon button next to all addresses and IDs
- Show a brief "Copied!" tooltip on click
- Use the Clipboard API with a fallback for older browsers

---

## Issue #33: UI/UX — Form validation errors are only shown on submit

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The registration and swap forms only validate on submit. Users fill out the entire form before learning about errors in earlier fields.

**Tasks:**
- Add inline validation on field blur for required fields and format checks
- Show a red border and error message below each invalid field
- Disable the submit button until all fields are valid

---

## Issue #34: UI/UX — No transaction history page

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Medium

**Estimated Time:** 2 hours

**Description:**
Users have no way to review their past swaps or registrations within the app. They must use an external block explorer to find their transaction history.

**Tasks:**
- Add a "My Activity" page showing past swaps (initiated, confirmed, cancelled) and registered listings
- Link each transaction hash to Stellar Expert
- Show the date, counterparty address, and USDC amount for each swap

---

## Issue #35: UI/UX — Accessibility: interactive elements lack focus styles

**Labels:** `ui/ux`

**Body:**

**Category:** UI/UX Design

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
Buttons and links in the UI do not have visible focus outlines. Keyboard-only users cannot tell which element is currently focused, making the app difficult to navigate without a mouse.

**Tasks:**
- Add a visible `:focus-visible` outline to all interactive elements
- Ensure focus order follows a logical reading order on all pages
- Test keyboard navigation through the full swap flow

---

## Issue #36: Frontend — Network selector (testnet / mainnet)

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Feature

**Priority:** Medium

**Estimated Time:** 1.5 hours

**Description:**
The frontend should allow switching between testnet and mainnet, loading the correct contract addresses for each network from environment config.

**Tasks:**
- Add a `NetworkSelector` component in the header
- Load contract addresses from `VITE_CONTRACT_*` env vars per network
- Warn users when connected to mainnet
- Persist network selection in `localStorage`

---

## Issue #37: Frontend — IPFS content preview for listings

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Feature

**Priority:** Low

**Estimated Time:** 3 hours

**Description:**
Buyers should be able to preview a description or metadata of an IP listing (stored on IPFS) before initiating a swap.

**Tasks:**
- Fetch listing metadata from IPFS using the `ipfs_hash` field via a public gateway
- Display title, description, and file type from the metadata JSON
- Handle IPFS fetch errors gracefully with a fallback UI
- Add a loading skeleton while fetching

---

## Issue #38: Frontend — ZK proof submission UI for sellers

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Feature

**Priority:** Medium

**Estimated Time:** 3 hours

**Description:**
Sellers need a UI to submit their Merkle root to the `zk_verifier` contract when registering IP, and to construct proof paths for verification.

**Tasks:**
- Create a `SetMerkleRootForm` component
- Accept a 32-byte hex root input and call `zk_verifier.set_merkle_root`
- Add a proof path builder UI for constructing `Vec<ProofNode>` inputs
- Show verification result from `verify_partial_proof`

---

## Issue #39: Frontend — Transaction status toasts and error handling

**Labels:** `frontend`

**Body:**

**Category:** Frontend - UX

**Priority:** High

**Estimated Time:** 2 hours

**Description:**
All contract interactions need user-facing feedback. Currently there is no frontend, so this must be built from scratch with a consistent notification system.

**Tasks:**
- Integrate a toast library (e.g. `react-hot-toast` or `sonner`)
- Show "Transaction submitted", "Transaction confirmed", and "Transaction failed" toasts
- Parse Soroban contract errors and display human-readable messages
- Add a transaction history panel showing recent tx hashes with Stellar Expert links

---

## Issue #40: Frontend — Responsive mobile layout

**Labels:** `frontend`

**Body:**

**Category:** Frontend - UX

**Priority:** Low

**Estimated Time:** 3 hours

**Description:**
The marketplace UI should be usable on mobile devices. Wallet interactions on mobile require deep-link support for Freighter mobile.

**Tasks:**
- Ensure all pages use responsive CSS (Tailwind or CSS modules)
- Test wallet connect flow on mobile browsers
- Add a mobile-friendly navigation drawer
- Verify swap forms are usable on small screens

---

## Issue #41: Frontend — Listing search and filter

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Feature

**Priority:** Medium

**Estimated Time:** 2 hours

**Description:**
As the number of listings grows, users need to search and filter the listings page by owner address, listing ID, or IPFS hash prefix.

**Tasks:**
- Add a search input to `ListingsPage` that filters by owner or listing ID
- Add a filter dropdown for listing status (available, swap pending, sold)
- Debounce search input to avoid excessive re-renders
- Persist filter state in URL query params

---

## Issue #42: Add `get_swap` function to `atomic_swap` for full swap detail retrieval

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The contract exposes `get_swap_status` and `get_decryption_key` but no function to retrieve the full `Swap` struct. Frontends must make multiple calls to reconstruct swap details.

**Tasks:**
- Add `get_swap(env, swap_id) -> Option<Swap>` to `AtomicSwap`
- Add a test asserting all fields are returned correctly
- Update frontend integration to use this single call

---

## Issue #43: `atomic_swap` does not validate that `zk_verifier` address is a valid contract

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`initiate_swap` accepts any `Address` as `zk_verifier` without verifying it is a deployed contract. A buyer could pass a random address, bypassing ZK verification entirely.

**Tasks:**
- Store a trusted `zk_verifier` address in `Config` set at initialization
- Remove the per-call `zk_verifier` parameter from `initiate_swap`
- Add a test that verifies only the configured verifier is used

---

## Issue #44: `ip_registry` does not emit events on registration

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`register_ip` does not emit any events. Off-chain indexers cannot discover new listings without polling all possible listing IDs.

**Tasks:**
- Emit a `ListingRegistered` event with `listing_id`, `owner`, `ipfs_hash`
- Add a test asserting the event is emitted with correct data

---

## Issue #45: `zk_verifier` does not emit an event when a Merkle root is set

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
`set_merkle_root` silently updates storage with no on-chain event. Indexers cannot track root updates.

**Tasks:**
- Emit a `MerkleRootSet` event with `listing_id` and `owner`
- Add a test asserting the event is emitted

---

## Issue #46: No integration test covering the full swap lifecycle end-to-end

**Labels:** `testing`

**Body:**

**Category:** Testing

**Priority:** High

**Estimated Time:** 3 hours

**Description:**
There are unit tests for individual functions but no single test that exercises the full flow: register IP → set Merkle root → initiate swap → verify proof → confirm swap → retrieve decryption key.

**Tasks:**
- Add `test_full_lifecycle` in `atomic_swap/src/lib.rs` using all three contracts
- Assert correct balances, swap status, and decryption key at each step
- Add a variant that tests the cancellation path

---

## Issue #47: `test.sh` script does not run tests with `--locked` flag

**Labels:** `devops`

**Body:**

**Category:** DevOps

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
CI should run tests with `cargo test --locked` to ensure `Cargo.lock` is respected and builds are reproducible.

**Tasks:**
- Update `scripts/test.sh` with `cargo test --locked --workspace`
- Add a `--nocapture` option for verbose output in CI
- Document usage in `README.md`

---

## Issue #48: Frontend — Contract address configuration via `.env`

**Labels:** `frontend`, `devops`

**Body:**

**Category:** Frontend - DevOps

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
The frontend needs to read deployed contract addresses from environment variables so the same build can target testnet or mainnet without code changes.

**Tasks:**
- Add `VITE_CONTRACT_IP_REGISTRY`, `VITE_CONTRACT_ATOMIC_SWAP`, `VITE_CONTRACT_ZK_VERIFIER` to `frontend/.env.example`
- Create a `contracts.ts` config module that reads these vars and throws if any are missing
- Document the setup in `README.md`

---

## Issue #49: `atomic_swap` fee calculation can silently truncate for small amounts

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The fee is calculated as `swap.usdc_amount * config.fee_bps as i128 / 10_000`. For small `usdc_amount` values, integer division truncates the fee to 0, meaning the fee recipient receives nothing even though a fee was configured.

**Tasks:**
- Add a minimum fee floor (e.g. 1 stroop) when `fee_bps > 0` and `usdc_amount > 0`
- Or document the truncation behavior explicitly with a comment
- Add a test with a small amount that verifies fee behavior at the boundary

---

## Issue #50: Frontend — Dark mode support

**Labels:** `frontend`

**Body:**

**Category:** Frontend - UX

**Priority:** Low

**Estimated Time:** 2 hours

**Description:**
The marketplace UI should respect the user's system dark mode preference and allow manual toggling.

**Tasks:**
- Add a `ThemeToggle` component using `prefers-color-scheme` media query
- Implement CSS variables or Tailwind dark mode classes for all components
- Persist theme preference in `localStorage`
- Ensure sufficient color contrast in both themes

---

## Issue #51: `atomic_swap` `ContractError` has duplicate discriminant values

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 1 hour

**Description:**
The `ContractError` enum defines `SwapNotPending = 6`, `SwapAlreadyPending = 7`, `SellerMismatch = 8`, and `SwapNotCancellable = 9` twice — once with values 6–9 and again with values 10–12 after `DisputeWindowExpired`. This causes duplicate discriminants which will fail to compile or produce incorrect error codes at runtime.

**Tasks:**
- Audit all `ContractError` variants and assign unique, sequential discriminant values
- Remove the duplicate block of variants
- Update all tests that match on specific error codes

---

## Issue #52: `atomic_swap` `SwapInitiated` event is published twice per `initiate_swap` call

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
`initiate_swap` calls `.publish(&env)` on two separate `SwapInitiated` struct literals — one using `usdc_amount` (which doesn't exist on `Swap`) and one using `amount`. This emits a duplicate event on every swap initiation and the first publish will fail to compile due to the wrong field name.

**Tasks:**
- Remove the first (incorrect) `SwapInitiated { ... usdc_amount }` publish block
- Keep only the correct `SwapInitiated { swap_id, listing_id, buyer, seller, amount }` publish
- Add a test asserting exactly one `SwapInitiated` event is emitted per call

---

## Issue #53: `atomic_swap` `release_to_seller` uses `swap.usdc_token` and `swap.usdc_amount` which don't exist on `Swap`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 1 hour

**Description:**
`release_to_seller` and `resolve_dispute` reference `swap.usdc_token` and `swap.usdc_amount`, but the `Swap` struct has fields named `token` and `amount`. This is a compile-time error that prevents the contract from building.

**Tasks:**
- Replace all `swap.usdc_token` references with `swap.token`
- Replace all `swap.usdc_amount` references with `swap.amount`
- Verify the contract compiles cleanly with `cargo build`

---

## Issue #54: `atomic_swap` `release_to_seller` uses `token_client` variable that is never declared

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 30 minutes

**Description:**
`release_to_seller` creates a `usdc` binding via `token::Client::new(...)` but then calls `token_client.transfer(...)` — `token_client` is never declared. Same issue exists in `resolve_dispute`. This is a compile error.

**Tasks:**
- Rename the `usdc` binding to `token_client` (or vice versa) consistently throughout both functions
- Ensure the contract compiles without undefined variable errors

---

## Issue #55: `atomic_swap` `release_to_seller` uses raw `assert!` instead of `panic_with_error!` for dispute window check

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`release_to_seller` uses `assert!(env.ledger().sequence() > confirmed_at + window, "dispute window has not yet expired")`. This produces an unstructured panic string instead of a typed `ContractError`, making it impossible for callers to match on the error programmatically.

**Tasks:**
- Add a `DisputeWindowActive` variant to `ContractError`
- Replace the `assert!` with `panic_with_error!(&env, ContractError::DisputeWindowActive)`
- Add a test that calls `release_to_seller` before the window expires and matches on the error

---

## Issue #56: `atomic_swap` `release_to_seller` uses `expect("confirmed_at_ledger missing")` instead of a structured error

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`release_to_seller` and `raise_dispute` call `.expect("confirmed_at_ledger missing")` on `swap.confirmed_at_ledger`. If the field is `None` (e.g. on a `Pending` swap), this panics with an unstructured string.

**Tasks:**
- Add a `MissingConfirmationLedger` variant to `ContractError`
- Replace both `.expect(...)` calls with `unwrap_or_else(|| panic_with_error!(&env, ContractError::MissingConfirmationLedger))`

---

## Issue #57: `atomic_swap` `initiate_swap` references `ContractError::InvalidToken` which is not defined

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 15 minutes

**Description:**
`initiate_swap` calls `env.panic_with_error(ContractError::InvalidToken)` when the token is not in the allowed list, but `InvalidToken` is not a variant in the `ContractError` enum. This is a compile error.

**Tasks:**
- Add `InvalidToken` variant to `ContractError` with a unique discriminant
- Add a test that passes a non-allowed token address and expects the error

---

## Issue #58: `atomic_swap` `Config` has `allowed_tokens: Vec<Address>` but `initialize` signature is missing the parameter

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
The `Config` struct includes `allowed_tokens: Vec<Address>` and `ip_registry: Address`, but the test helper `setup_full` calls `client.initialize(&admin, &0u32, &fee_recipient, &60u64)` with only 4 arguments, omitting `ip_registry` and `allowed_tokens`. The `initialize` function signature must match all `Config` fields.

**Tasks:**
- Update all test `initialize` calls to pass `ip_registry` and `allowed_tokens`
- Ensure the `initialize` function signature matches the `Config` struct exactly
- Update `setup_full` helper accordingly

---

## Issue #59: `atomic_swap` `cancel_swap` emits both a raw `env.storage().events().publish` and a typed `SwapCancelled` event

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`cancel_swap` calls `env.storage().events().publish(...)` (the old low-level API) and then also calls `SwapCancelled { ... }.publish(&env)`. This emits two events per cancellation and the first uses the deprecated storage-events API.

**Tasks:**
- Remove the `env.storage().events().publish(...)` call
- Keep only the typed `SwapCancelled { swap_id, buyer, amount }.publish(&env)`
- Add a test asserting exactly one cancellation event is emitted

---

## Issue #60: `atomic_swap` `release_to_seller` does not emit any event after releasing funds

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`release_to_seller` transfers funds to the seller but emits no on-chain event. Off-chain indexers cannot detect when a swap is fully settled without polling swap status.

**Tasks:**
- Define a `FundsReleased { swap_id, seller, amount }` event struct
- Emit it at the end of `release_to_seller`
- Add a test asserting the event is emitted with correct values

---

## Issue #61: `atomic_swap` `resolve_dispute` does not emit any event

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`resolve_dispute` changes swap status and transfers funds but emits no event. Dispute outcomes are invisible to off-chain systems.

**Tasks:**
- Define a `DisputeResolved { swap_id, favor_buyer: bool }` event struct
- Emit it at the end of `resolve_dispute`
- Add a test for both resolution paths asserting the event is emitted

---

## Issue #62: `atomic_swap` `raise_dispute` does not emit any event

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
`raise_dispute` transitions the swap to `Disputed` status silently. There is no on-chain event for indexers or the seller to react to.

**Tasks:**
- Define a `DisputeRaised { swap_id, buyer }` event struct
- Emit it at the end of `raise_dispute`
- Add a test asserting the event is emitted

---

## Issue #63: `atomic_swap` `ActiveListingSwap` is not cleared after `cancel_swap`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
`cancel_swap` sets the swap status to `Cancelled` but never removes the `DataKey::ActiveListingSwap(listing_id)` entry. After cancellation, `is_listing_available` will still find the stale entry and check the cancelled swap's status, which returns `true` only because `Cancelled != Pending`. However, a new `initiate_swap` will also find the stale entry and attempt to load the old swap, potentially causing confusion or incorrect duplicate-swap rejection logic.

**Tasks:**
- Add `env.storage().persistent().remove(&DataKey::ActiveListingSwap(swap.listing_id))` in `cancel_swap`
- Add a test that cancels a swap and then successfully initiates a new swap on the same listing

---

## Issue #64: `ip_registry` `Listing` struct is missing `price`, `royalty_bps`, and `royalty_recipient` fields used by `atomic_swap`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 2 hours

**Description:**
`atomic_swap`'s `release_to_seller` reads `listing.royalty_bps` and `listing.royalty_recipient` from the `Listing` struct returned by `IpRegistryClient`. However, the `ip_registry` `Listing` struct only has `owner`, `ipfs_hash`, and `merkle_root`. The `register_ip` function also has no `price`, `royalty_bps`, or `royalty_recipient` parameters. This is a cross-contract type mismatch that will fail at compile time.

**Tasks:**
- Add `price: i128`, `royalty_bps: u32`, and `royalty_recipient: Address` to the `Listing` struct in `ip_registry`
- Update `register_ip` and `batch_register_ip` to accept and store these fields
- Update all existing tests to pass the new required fields
- Update `setup_registry` test helper in `atomic_swap` tests accordingly

---

## Issue #65: `ip_registry` `register_ip` does not validate that `price > 0`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
Once `price` is added to the `Listing` struct (see #64), `register_ip` must validate that `price > 0`. A listing with zero or negative price would allow buyers to initiate swaps for free.

**Tasks:**
- Add `if price <= 0 { return Err(ContractError::InvalidInput); }` in `register_ip`
- Add `InvalidPrice` variant to `ContractError` for a more descriptive error
- Add tests for zero and negative price inputs

---

## Issue #66: `ip_registry` `register_ip` does not validate `royalty_bps <= 10_000`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
Once `royalty_bps` is added (see #64), there is no upper-bound check. A royalty of more than 10,000 bps (100%) would cause the seller to receive a negative amount after fee and royalty deductions.

**Tasks:**
- Add `if royalty_bps > 10_000 { return Err(ContractError::InvalidInput); }` in `register_ip`
- Add a test with `royalty_bps = 10_001` expecting the error
- Add a test with `royalty_bps = 10_000` (100%) succeeding

---

## Issue #67: `atomic_swap` royalty deduction in `release_to_seller` mutates an immutable `let` binding

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 20 minutes

**Description:**
`release_to_seller` declares `let seller_amount = swap.usdc_amount - fee;` and then later does `seller_amount -= royalty;`. In Rust, `let` bindings are immutable by default, so this is a compile error.

**Tasks:**
- Change `let seller_amount` to `let mut seller_amount` in `release_to_seller`
- Apply the same fix in `resolve_dispute` if it has the same pattern
- Verify the contract compiles

---

## Issue #68: `zk_verifier` `verify_partial_proof` panics on missing root instead of returning `false`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
`verify_partial_proof` calls `.expect("root not found")` when no Merkle root is stored for a listing. This causes a hard panic rather than returning `false`, breaking any caller that expects a boolean result for an unregistered listing.

**Tasks:**
- Replace `.expect("root not found")` with an early `return false` when the root is `None`
- Add a test that calls `verify_partial_proof` on a listing with no stored root and asserts `false`

---

## Issue #69: `zk_verifier` has no `transfer_root_ownership` function

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 1 hour

**Description:**
Once a Merkle root is set for a listing, the `Owner(listing_id)` entry is permanent and cannot be transferred. If the listing ownership changes (see #15), the new owner cannot update the Merkle root.

**Tasks:**
- Add `transfer_root_ownership(env, current_owner, listing_id, new_owner)` requiring auth from `current_owner`
- Update `DataKey::Owner(listing_id)` to the new owner
- Add tests for successful transfer and unauthorized attempt

---

## Issue #70: `zk_verifier` `set_merkle_root` does not extend TTL on the `Owner` key when overwriting the root

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
When `set_merkle_root` is called a second time by the same owner (to update the root), it only extends TTL for the `MerkleRoot` key. The `Owner` key TTL is only extended on first write. If the owner key expires before the root key, a new caller could claim ownership of the listing in `zk_verifier`.

**Tasks:**
- Call `extend_ttl` on `DataKey::Owner(listing_id)` on every successful `set_merkle_root` call, not just on first write
- Add a test that updates the root after a large ledger advance and verifies the owner key persists

---

## Issue #71: `atomic_swap` `get_swaps_by_seller` has no corresponding test

**Labels:** `testing`

**Body:**

**Category:** Testing

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
`get_swaps_by_seller` is implemented and exported but has no test coverage. The seller index could silently break without detection.

**Tasks:**
- Add `test_get_swaps_by_seller_empty` asserting an empty vec for an unknown seller
- Add `test_get_swaps_by_seller_single` asserting the swap ID appears after `initiate_swap`
- Add `test_get_swaps_by_seller_multiple` with two swaps on different listings

---

## Issue #72: `atomic_swap` `is_listing_available` has no test coverage

**Labels:** `testing`

**Body:**

**Category:** Testing

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
`is_listing_available` is a public view function with no tests. Edge cases like a listing with a cancelled swap or a completed swap are untested.

**Tasks:**
- Add `test_is_listing_available_no_swap` asserting `true` for a listing with no swap
- Add `test_is_listing_available_pending_swap` asserting `false` while a swap is pending
- Add `test_is_listing_available_after_cancel` asserting `true` after cancellation

---

## Issue #73: `ip_registry` `listing_count` has no test for the zero state

**Labels:** `testing`

**Body:**

**Category:** Testing

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
`listing_count` returns `0` before any registrations, but this is only implicitly tested inside `test_listing_count`. There is no dedicated snapshot test for the zero state, making it easy to miss a regression where the counter starts at a non-zero value.

**Tasks:**
- Add `test_listing_count_zero_initial` as a standalone snapshot test
- Verify the snapshot captures `0` correctly

---

## Issue #74: `atomic_swap` `set_dispute_window` has no lower-bound validation

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`set_dispute_window` accepts any `u32` value including `0`. A dispute window of 0 ledgers means buyers can never raise a dispute because the window expires immediately after confirmation.

**Tasks:**
- Add a minimum value check (e.g. `if ledgers < 100 { panic_with_error!(...) }`)
- Add `DisputeWindowTooShort` to `ContractError`
- Add a test that passes `0` and expects the error

---

## Issue #75: `atomic_swap` `Config.cancel_delay_secs` is used as both a cancel delay and a swap expiry

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`initiate_swap` sets `expires_at = now + config.cancel_delay_secs`, meaning the cancel delay and the swap expiry are the same value. A very short cancel delay also means the swap expires quickly, potentially before the seller has time to confirm. These should be separate config fields.

**Tasks:**
- Add `swap_expiry_secs: u64` to `Config` as a separate field from `cancel_delay_secs`
- Use `swap_expiry_secs` for `expires_at` in `initiate_swap`
- Use `cancel_delay_secs` only in the cancel delay check (once issue #2 is fixed)
- Update `initialize` and all tests accordingly

---

## Issue #76: `ip_registry` `batch_register_ip` re-reads and re-writes the `OwnerIndex` on every iteration

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
Inside the batch loop, `batch_register_ip` reads `DataKey::OwnerIndex(owner)` from storage, pushes one ID, and writes it back on every iteration. For a batch of N entries this is O(N) storage reads and writes for the index alone, which is expensive in Soroban's metered environment.

**Tasks:**
- Load the owner index once before the loop
- Push all IDs inside the loop without writing
- Write the final index once after the loop
- Add a benchmark comment noting the improvement

---

## Issue #77: `atomic_swap` `initiate_swap` does not verify the listing exists before cross-calling `ip_registry`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`initiate_swap` calls `IpRegistryClient::new(&env, &ip_registry).get_listing(&listing_id)` and immediately accesses `.owner` without checking if the listing is `Some`. If the listing does not exist, `get_listing` returns `None` and the `.owner` access will panic with an unhelpful message.

**Tasks:**
- Unwrap the `Option<Listing>` with `unwrap_or_else(|| panic_with_error!(&env, ContractError::SwapNotFound))` or add a `ListingNotFound` error variant
- Add a test that initiates a swap with a non-existent listing ID and expects the error

---

## Issue #78: `atomic_swap` `initiate_swap` accepts a caller-supplied `ip_registry` address instead of using the one stored in `Config`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`initiate_swap` takes both `zk_verifier: Address` and `ip_registry: Address` as caller-supplied parameters, but `Config` already stores a trusted `ip_registry` address set at initialization. A malicious buyer could pass a fake registry that always returns a listing owned by the seller, bypassing ownership verification.

**Tasks:**
- Remove the `ip_registry` parameter from `initiate_swap`
- Read `config.ip_registry` from storage instead
- Update all tests and the frontend integration

---

## Issue #79: `zk_verifier` proof verification does not check that the leaf hash length matches `BytesN<32>`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`verify_partial_proof` hashes the raw `leaf: Bytes` with SHA-256 to get a 32-byte value, which is correct. However, there is no validation that the `path` vector's `sibling` fields are non-zero or that the path length is reasonable. An empty sibling hash (`[0u8; 32]`) could be used to forge proofs against a root that was constructed with zero-padded nodes.

**Tasks:**
- Add a check that rejects any `ProofNode` where `sibling == BytesN::from_array(&env, &[0u8; 32])`
- Add a `MaxProofDepth` constant (e.g. 64) and reject paths longer than this
- Add tests for zero-sibling and oversized path inputs

---

## Issue #80: `atomic_swap` has no `get_config` view function

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
There is no public function to read the current `Config` (fee_bps, fee_recipient, cancel_delay_secs, ip_registry, allowed_tokens). Frontends and integrators must guess or hardcode these values.

**Tasks:**
- Add `get_config(env) -> Option<Config>` to `AtomicSwap`
- Add a test asserting the returned config matches what was passed to `initialize`

---

## Issue #81: `atomic_swap` `Config` does not store `ip_registry` address — `initiate_swap` still accepts it as a caller-supplied parameter

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
The `Config` struct only stores `fee_bps`, `fee_recipient`, `cancel_delay_secs`, and `zk_verifier`. There is no `ip_registry` field. Yet `initiate_swap` accepts `ip_registry: Address` as a caller-supplied parameter, meaning any buyer can point to a fake registry. The `zk_verifier` address is stored in `Config` but also accepted per-call via the `Swap` struct, creating an inconsistency.

**Tasks:**
- Add `ip_registry: Address` to `Config` and set it in `initialize`
- Remove the `ip_registry` parameter from `initiate_swap` and read from `config.ip_registry`
- Remove the per-call `zk_verifier` parameter from `initiate_swap` and read from `config.zk_verifier`
- Update all tests and the `setup_full` helper accordingly

---

## Issue #82: `atomic_swap` `release_to_seller` uses raw `assert!` for dispute window check instead of `panic_with_error!`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`release_to_seller` contains `assert!(env.ledger().sequence() > confirmed_at + window, "dispute window has not yet expired")`. This produces an unstructured panic string rather than a typed `ContractError`, making it impossible for callers to programmatically match on the error.

**Tasks:**
- Add a `DisputeWindowActive` variant to `ContractError` with a unique discriminant
- Replace the `assert!` with `panic_with_error!(&env, ContractError::DisputeWindowActive)`
- Add a test that calls `release_to_seller` before the window expires and matches on `DisputeWindowActive`

---

## Issue #83: `atomic_swap` `raise_dispute` and `release_to_seller` use `.expect("confirmed_at_ledger missing")` instead of a structured error

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
Both `raise_dispute` and `release_to_seller` call `swap.confirmed_at_ledger.expect("confirmed_at_ledger missing")`. If the field is `None` (e.g. on a swap that was never confirmed), this panics with an unstructured string instead of a typed `ContractError`.

**Tasks:**
- Add a `MissingConfirmationLedger` variant to `ContractError`
- Replace both `.expect(...)` calls with `unwrap_or_else(|| panic_with_error!(&env, ContractError::MissingConfirmationLedger))`
- Add a test that calls `raise_dispute` on a `Pending` swap and expects the structured error

---

## Issue #84: `atomic_swap` `cancel_swap` does not remove `ActiveListingSwap` entry after cancellation

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
`cancel_swap` sets `swap.status = SwapStatus::Cancelled` and writes the swap back to storage, but never removes the `DataKey::ActiveListingSwap(listing_id)` entry. After cancellation, `is_listing_available` still finds the stale entry and loads the cancelled swap. While it currently returns `true` (because `Cancelled != Pending`), a future `initiate_swap` will find the stale `ActiveListingSwap` key and attempt to load the old swap, potentially triggering the duplicate-swap rejection path incorrectly.

**Tasks:**
- Add `env.storage().persistent().remove(&DataKey::ActiveListingSwap(swap.listing_id))` inside `cancel_swap` after updating the swap status
- Add a test that cancels a swap and then successfully initiates a new swap on the same listing

---

## Issue #85: `atomic_swap` `get_swap` and `get_decryption_key` do not extend TTL on read

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
`get_swap` and `get_decryption_key` read from persistent storage without extending the TTL of the swap entry. A swap that is frequently read but not written (e.g. a completed swap whose decryption key is being retrieved) will eventually expire and become unreadable.

**Tasks:**
- Call `extend_ttl` on `DataKey::Swap(swap_id)` inside both `get_swap` and `get_decryption_key` when the entry exists
- Add a test that reads a swap near TTL expiry and verifies it persists

---

## Issue #86: `atomic_swap` `get_swaps_by_buyer_page` panics when `offset == total` instead of returning an empty page

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`get_swaps_by_buyer_page` panics with `InvalidPaginationParams` when `offset > total`. However, `offset == total` is a valid cursor-past-end state that should return an empty `Vec` rather than an error. Callers iterating through pages will hit this panic on the final page boundary.

**Tasks:**
- Change the guard to `if offset > total` → return empty `Vec` (not panic) when `offset == total`
- Keep the panic only for `offset > total` or `limit == 0`
- Update the existing `test_get_swaps_by_buyer_page_offset_at_end` test to reflect the corrected behavior

---

## Issue #87: `ip_registry` `IpEntry` type alias does not include `price_usdc`, `royalty_bps`, or `royalty_recipient`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`IpEntry` is defined as `pub type IpEntry = (Bytes, Bytes)` — a tuple of `(ipfs_hash, merkle_root)`. But the `Listing` struct now has `price_usdc: i128`, `royalty_bps: u32`, and `royalty_recipient: Address`. `batch_register_ip` uses `IpEntry` and therefore cannot accept or store these new fields, making batch registration produce incomplete listings.

**Tasks:**
- Redefine `IpEntry` as a struct with all `Listing` fields except `owner` (which is passed separately)
- Update `batch_register_ip` to read and store all fields from the new `IpEntry`
- Update all tests that construct `IpEntry` values

---

## Issue #88: `ip_registry` `update_listing` does not validate that updated `ipfs_hash` and `merkle_root` are non-empty

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`update_listing` allows callers to overwrite `ipfs_hash` and `merkle_root` with empty `Bytes`. The same validation that `register_ip` applies (rejecting empty values) is absent in `update_listing`, creating an inconsistency that could corrupt a listing.

**Tasks:**
- Add `if ipfs_hash.is_empty() || merkle_root.is_empty() { panic_with_error!(&env, ContractError::InvalidInput) }` in `update_listing`
- Add tests for empty `ipfs_hash` and empty `merkle_root` inputs to `update_listing`

---

## Issue #89: `ip_registry` `deregister_listing` does not check for a pending swap before removing the listing

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`deregister_listing` removes a listing from storage without checking whether there is an active pending swap against it. If a buyer has already initiated a swap and the seller deregisters the listing, the swap's `confirm_swap` call will still succeed (the listing is no longer needed after initiation), but `release_to_seller` will fail to read royalty data from the now-deleted listing.

**Tasks:**
- Accept an optional `atomic_swap: Address` parameter in `deregister_listing` and call `AtomicSwapClient::has_pending_swap` before removing
- Return `ContractError::PendingSwapExists` if a pending swap is found
- Add a test that attempts deregistration while a swap is pending and expects the error

---

## Issue #90: `ip_registry` `Config` is stored in instance storage but listings are in persistent storage — TTL mismatch risk

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 45 minutes

**Description:**
`ip_registry` stores its `Config` (including TTL parameters) in instance storage, while all `Listing` entries are in persistent storage. If instance storage expires, `get_config` will panic with `NotInitialized` even though all listings are still alive in persistent storage. The contract becomes permanently broken until re-initialized.

**Tasks:**
- Move `Config` to persistent storage with its own TTL extension on every write
- Or extend instance storage TTL on every `register_ip` / `update_listing` call
- Add a test that advances the ledger past the instance TTL and verifies `get_listing` still works

---

## Issue #91: `zk_verifier` `verify_partial_proof` does not validate that `proof_path` length is within a safe bound

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`verify_partial_proof` iterates over the entire `proof_path` Vec without any length cap. A malicious caller could pass a path with thousands of nodes, consuming excessive CPU instructions and causing the transaction to hit Soroban's instruction limit, effectively DoS-ing the contract for that call.

**Tasks:**
- Add a `MAX_PROOF_DEPTH: u32 = 64` constant
- Reject paths longer than `MAX_PROOF_DEPTH` with `ContractError::InvalidInput`
- Add a test with an oversized path that expects the error

---

## Issue #92: `zk_verifier` has no `get_owner` view function

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
There is no public function to query who owns the Merkle root for a given listing in `zk_verifier`. Frontends and other contracts must call `set_merkle_root` and catch the unauthorized error to infer ownership, which is wasteful and error-prone.

**Tasks:**
- Add `get_owner(env, listing_id) -> Option<Address>` to `ZkVerifier`
- Add a test asserting the correct owner is returned after `set_merkle_root`
- Add a test asserting `None` is returned for a listing with no root

---

## Issue #93: Frontend — `ConfirmSwapForm` does not accept or submit a ZK proof path

**Labels:** `frontend`, `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** High

**Estimated Time:** 2 hours

**Description:**
`confirm_swap` on the contract now requires a `proof_path: Vec<ProofNode>` argument in addition to the `decryption_key`. The `ConfirmSwapForm` component only collects the decryption key and calls `confirmSwap(swap.id, decryptionKey, wallet)`. The `confirmSwap` function in `contractClient.ts` does not pass a `proof_path`, so every confirmation will fail with an arity mismatch or an invalid proof error.

**Tasks:**
- Add a `proof_path` input field (JSON or hex-encoded nodes) to `ConfirmSwapForm`
- Update `confirmSwap` in `contractClient.ts` to accept and serialize `proof_path` as `Vec<ProofNode>` ScVal
- Add client-side validation that the proof path is non-empty before submitting
- Document the expected proof path format in a comment

---

## Issue #94: Frontend — `contractClient.ts` `decodeSwapScVal` reads `native.usdc_amount` but the contract field is `usdc_amount` — verify field name alignment

**Labels:** `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`decodeSwapScVal` in `contractClient.ts` reads `native.usdc_amount` to populate the `usdc_amount` field. The `Swap` struct in the contract also uses `usdc_amount`, so this is currently correct. However, the `Swap` struct also has a `usdc_token` field that is never decoded, and the frontend `Swap` interface in `useMySwaps.ts` has no `token` field at all. If the contract field names change (as they have in past refactors), the frontend will silently return `0` for amounts.

**Tasks:**
- Add a `token: string` field to the frontend `Swap` interface in `useMySwaps.ts`
- Decode `native.usdc_token` in `decodeSwapScVal` and include it in the returned object
- Add a runtime assertion or TypeScript type guard that `usdc_amount > 0` for non-void swap results

---

## Issue #95: Frontend — `useMySwaps` and `useMyListings` poll every 15 s unconditionally, even when the tab is hidden

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Performance

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
Both `useMySwaps` and `useMyListings` set a 15-second `setInterval` that fires regardless of whether the browser tab is visible. This wastes RPC calls and battery on mobile when the user has switched away from the tab.

**Tasks:**
- Add a `visibilitychange` event listener that pauses the interval when `document.hidden` is `true` and resumes it when the tab becomes visible again
- Apply the fix to both `useMySwaps.ts` and `useMyListings.js`
- Add a comment explaining the optimization

---

## Issue #96: Frontend — `MyListingsDashboard` uses `.jsx` while all other new components use `.tsx`

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Code Quality

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
`MyListingsDashboard.jsx` and `useMyListings.js` are plain JavaScript files while every other new component (`WalletConnectButton.tsx`, `MySwapsDashboard.tsx`, `ConfirmSwapForm.tsx`, etc.) uses TypeScript. This inconsistency means `MyListingsDashboard` has no type checking, and the `listing` objects it passes to `ListingCard` are untyped.

**Tasks:**
- Rename `MyListingsDashboard.jsx` → `MyListingsDashboard.tsx` and add prop/return types
- Rename `useMyListings.js` → `useMyListings.ts` and add full TypeScript types
- Define a `Listing` interface in a shared types file and use it in both hooks and components

---

## Issue #97: Frontend — `WalletContext` silently swallows the auto-reconnect error, leaving `connecting: true` on failure

**Labels:** `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
In `WalletProvider`, the auto-reconnect on mount calls `connectWallet(savedId).catch(() => localStorage.removeItem(STORAGE_KEY))`. If `connectWallet` throws, the error is swallowed and `setConnecting(false)` is called in `.finally()`. However, if the promise rejects before `.finally()` runs (e.g. due to a synchronous throw), `connecting` could remain `true` indefinitely. More importantly, the user sees no feedback that reconnection failed.

**Tasks:**
- Add `.catch((err) => { setError(err instanceof Error ? err.message : "Auto-reconnect failed."); localStorage.removeItem(STORAGE_KEY); })` in the mount effect
- Ensure `setConnecting(false)` is always called in `.finally()`
- Add a visible "Reconnection failed" state to `WalletConnectButton` when `error` is set on mount

---

## Issue #98: Frontend — `InitiateSwapModal` passes `ZK_VERIFIER_CONTRACT_ID` as a parameter to `initiateSwap` but the contract no longer accepts it per-call

**Labels:** `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`InitiateSwapModal.tsx` calls `initiateSwap(listing.id, listing.owner, USDC_CONTRACT_ID, raw, ZK_VERIFIER_CONTRACT_ID, wallet)`. Once issue #81 is resolved and `zk_verifier` is removed from the `initiate_swap` parameter list, this call will pass an extra argument that the contract does not accept, causing the transaction to fail. The frontend must be updated in lockstep.

**Tasks:**
- Remove `ZK_VERIFIER_CONTRACT_ID` from the `initiateSwap` call in `InitiateSwapModal.tsx`
- Update the `initiateSwap` function signature in `contractClient.ts` to remove the `zkVerifier` parameter
- Remove the `VITE_CONTRACT_ZK_VERIFIER` read from `InitiateSwapModal.tsx` (it will no longer be needed here)

---

## Issue #99: Frontend — `contractClient.ts` `simulateIpRegistryView` is missing TypeScript parameter types

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Code Quality

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
`simulateIpRegistryView(functionName, args)` and several IP registry helper functions (`decodeListingScVal`, `getListingsByOwner`, `getListing`, `getSwapsBySeller`) are missing TypeScript parameter and return type annotations. The file is a `.ts` file so these should be typed.

**Tasks:**
- Add `functionName: string` and `args: StellarSdk.xdr.ScVal[]` types to `simulateIpRegistryView`
- Add return type `Promise<StellarSdk.xdr.ScVal | undefined>` to `simulateIpRegistryView`
- Add typed return types to `getListingsByOwner`, `getListing`, `getSwapsBySeller`
- Define and export a `Listing` TypeScript interface matching the decoded shape

---

## Issue #100: Frontend — No `RegisterListingForm` component exists for sellers to register new IP

**Labels:** `frontend`

**Body:**

**Category:** Frontend - Feature

**Priority:** High

**Estimated Time:** 3 hours

**Description:**
There is no UI for sellers to register a new IP listing. `MyListingsDashboard` shows existing listings but provides no way to create one. The `register_ip` contract function is never called from the frontend.

**Tasks:**
- Create `RegisterListingForm.tsx` with fields for `ipfs_hash`, `merkle_root`, `price_usdc`, `royalty_bps`, and `royalty_recipient`
- Add a `registerIp` function to `contractClient.ts` that builds and submits the `register_ip` transaction
- Add client-side validation (non-empty hashes, `price > 0`, `royalty_bps <= 10000`)
- Integrate the form into `MyListingsDashboard` behind an "Register New IP" button

---

## Issue #101: `ip_registry` `register_ip` validates `price_usdc < 0` and `price_usdc <= 0` in two separate guards, making the first check dead code

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
`register_ip` has this guard at the top:
```rust
if ipfs_hash.is_empty() || merkle_root.is_empty() || price_usdc < 0 || royalty_bps > 10_000 {
    return Err(ContractError::InvalidInput);
}
if price_usdc <= 0 {
    return Err(ContractError::InvalidPrice);
}
```
The first `if` catches `price_usdc < 0` and returns `InvalidInput`. The second `if` catches `price_usdc <= 0` and returns `InvalidPrice`. Because the first guard fires first for negative values, callers passing a negative price receive `InvalidInput` instead of the more descriptive `InvalidPrice`. The `price_usdc < 0` branch in the first guard is dead code.

**Tasks:**
- Remove `price_usdc < 0` from the combined guard
- Let the `price_usdc <= 0` check below handle all non-positive prices with `InvalidPrice`
- Add a test asserting that a negative price returns `InvalidPrice` (not `InvalidInput`)

---

## Issue #102: `ip_registry` `batch_register_ip` hardcodes `royalty_bps: 0`, `royalty_recipient: owner.clone()`, and `price_usdc: 0` for every entry

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 2 hours

**Description:**
`batch_register_ip` creates each `Listing` with `royalty_bps: 0`, `royalty_recipient: owner.clone()`, and `price_usdc: 0` regardless of what the caller wants. This means batch-registered listings always have zero price and no royalty, making them unusable for actual sales. The `IpEntry` type alias `(Bytes, Bytes)` does not carry these fields, so there is no way to pass them.

**Tasks:**
- Define `IpEntry` as a struct: `{ ipfs_hash, merkle_root, price_usdc, royalty_bps, royalty_recipient }`
- Update `batch_register_ip` to read and store all fields from each `IpEntry`
- Add the same `price_usdc > 0` and `royalty_bps <= 10_000` validation per entry
- Update all tests that construct `IpEntry` values

---

## Issue #103: `ip_registry` `batch_register_ip` does not validate `price_usdc` or `royalty_bps` per entry

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
The validation loop in `batch_register_ip` only checks `ipfs_hash.is_empty() || merkle_root.is_empty()`. It does not validate `price_usdc > 0` or `royalty_bps <= 10_000` per entry, creating an inconsistency with `register_ip` which enforces both. Once `IpEntry` is extended (see #102), these checks must be added.

**Tasks:**
- Add `price_usdc <= 0` and `royalty_bps > 10_000` checks inside the validation loop
- Return `ContractError::InvalidPrice` or `ContractError::InvalidInput` accordingly
- Add tests for each invalid case in a batch

---

## Issue #104: `ip_registry` `update_listing` does not update `price_usdc` or `royalty_bps`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`update_listing` only accepts `new_ipfs_hash` and `new_merkle_root`. Sellers cannot change the price or royalty of a listing after registration without deregistering and re-registering, which would assign a new listing ID and break any existing references.

**Tasks:**
- Add `new_price_usdc: i128` and `new_royalty_bps: u32` parameters to `update_listing`
- Apply the same `price_usdc > 0` and `royalty_bps <= 10_000` validation
- Update all tests for `update_listing`
- Emit an `IpUpdated` event with the changed fields

---

## Issue #105: `ip_registry` `deregister_listing` does not remove the listing from the `OwnerIndex` when the owner has only one listing

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`deregister_listing` uses `(0..ids.len()).find(|&i| ids.get(i).unwrap() == listing_id)` to locate the listing in the owner index and calls `ids.remove(pos)`. If the owner has exactly one listing and it is deregistered, `ids` becomes empty but is still written back to storage as an empty `Vec`. Subsequent calls to `list_by_owner` return an empty vec (correct), but the empty `OwnerIndex` key persists in storage indefinitely, wasting storage budget.

**Tasks:**
- After removing the ID, check `if ids.is_empty()` and call `env.storage().persistent().remove(&idx_key)` instead of writing back an empty vec
- Add a test that deregisters the only listing and verifies the owner index key is gone

---

## Issue #106: `ip_registry` `transfer_listing_ownership` does not check for a pending swap before transferring

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`transfer_listing_ownership` changes the listing owner without checking whether there is an active pending swap. If a buyer has already initiated a swap against the listing, the seller field in the `Swap` struct will no longer match the new listing owner, causing `SellerMismatch` errors or allowing the new owner to confirm a swap they did not agree to.

**Tasks:**
- Accept an optional `atomic_swap: Address` parameter and call `AtomicSwapClient::has_pending_swap` before transferring
- Return `ContractError::PendingSwapExists` if a pending swap is found
- Add a test that attempts transfer while a swap is pending and expects the error

---

## Issue #107: `ip_registry` `update_listing` does not check for a pending swap before updating hashes

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 45 minutes

**Description:**
`update_listing` allows the seller to change `ipfs_hash` and `merkle_root` while a buyer's swap is pending. If the seller updates the Merkle root mid-swap, the buyer's ZK proof (which was constructed against the old root) will fail verification when `confirm_swap` is called, locking the buyer's USDC until the swap expires.

**Tasks:**
- Accept an optional `atomic_swap: Address` parameter and call `AtomicSwapClient::has_pending_swap` before updating
- Return `ContractError::PendingSwapExists` if a pending swap is found
- Add a test that attempts an update while a swap is pending and expects the error

---

## Issue #108: `atomic_swap` `initiate_swap` calls `calculate_fee_amount` but discards the result — fee is not validated at initiation time

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`initiate_swap` calls `Self::calculate_fee_amount(&env, usdc_amount, config.fee_bps)` but does not use the return value. The call is only there to trigger the `FeeWouldTruncate` panic if the fee would be zero. This is a silent side-effect call with no documentation, and the result is thrown away. If `calculate_fee_amount` is ever refactored to not panic, the validation silently disappears.

**Tasks:**
- Assign the result: `let _fee = Self::calculate_fee_amount(...)` with a comment explaining the intent
- Or extract the truncation check into a dedicated `assert_fee_valid` helper that makes the intent explicit
- Add a comment explaining why the result is discarded

---

## Issue #109: `atomic_swap` `resolve_dispute` silently falls back to sending full amount to seller when `Config` is missing

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
In `resolve_dispute`, the `favor_buyer = false` branch does:
```rust
if let Some(config) = env.storage().instance().get::<DataKey, Config>(&DataKey::Config) {
    // deduct fee and pay seller
} else {
    usdc.transfer(&contract_addr, &swap.seller, &swap.usdc_amount);
}
```
If `Config` is somehow missing (e.g. instance storage expired), the contract silently sends the full amount to the seller without deducting the protocol fee. This is an inconsistency with `release_to_seller` which panics with `NotInitialized` in the same scenario.

**Tasks:**
- Replace the `if let Some(config)` with `unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized))`
- Remove the silent fallback branch
- Add a test verifying `resolve_dispute` panics with `NotInitialized` when config is absent

---

## Issue #110: `atomic_swap` `release_to_seller` does not deduct royalties before paying the seller

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 2 hours

**Description:**
`release_to_seller` deducts the protocol fee and sends the remainder to the seller, but never reads `listing.royalty_bps` or `listing.royalty_recipient` from `ip_registry`. Royalty recipients receive nothing even when a non-zero royalty was set at registration time. This is a core marketplace invariant violation.

**Tasks:**
- Cross-call `IpRegistryClient::get_listing` inside `release_to_seller` to read `royalty_bps` and `royalty_recipient`
- Compute `royalty = (seller_amount * royalty_bps) / 10_000` and deduct it from `seller_amount`
- Transfer royalty to `royalty_recipient` if `royalty > 0`
- Add a test that verifies royalty is paid correctly alongside the protocol fee

---

## Issue #111: `atomic_swap` `resolve_dispute` does not deduct royalties when resolving in favor of the seller

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`resolve_dispute` with `favor_buyer = false` deducts the protocol fee but does not deduct royalties, same root cause as #110. A dispute resolved in the seller's favor should follow the same payment split as `release_to_seller`.

**Tasks:**
- Apply the same royalty deduction logic from #110 to the `favor_buyer = false` branch of `resolve_dispute`
- Add a test verifying royalty is paid when a dispute is resolved in the seller's favor

---

## Issue #112: `atomic_swap` `Swap` struct stores `zk_verifier: Address` per-swap but `Config` already stores a trusted `zk_verifier`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The `Swap` struct has a `zk_verifier: Address` field that is set from the caller-supplied parameter in `initiate_swap`. `confirm_swap` then uses `swap.zk_verifier` to call the verifier. Since `Config` already stores a trusted `zk_verifier`, storing it again per-swap wastes storage and allows a buyer to embed a malicious verifier address in the swap at initiation time.

**Tasks:**
- Remove `zk_verifier: Address` from the `Swap` struct
- In `confirm_swap`, read `config.zk_verifier` from instance storage instead of `swap.zk_verifier`
- Update all tests that construct `Swap` values or call `initiate_swap` with a `zk_verifier` argument

---

## Issue #113: `zk_verifier` `get_merkle_root` does not extend TTL on read

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
`get_merkle_root` reads from persistent storage without extending the TTL of the `MerkleRoot` key. A root that is frequently queried but never updated will eventually expire, causing `verify_partial_proof` to return `false` for a valid listing.

**Tasks:**
- Call `extend_ttl` on `DataKey::MerkleRoot(listing_id)` inside `get_merkle_root` when the entry exists
- Add a test that reads a root near TTL expiry and verifies it persists

---

## Issue #114: `zk_verifier` `transfer_root_ownership` does not emit an event

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
`transfer_root_ownership` silently updates the `Owner` key with no on-chain event. Off-chain indexers cannot track ownership changes for Merkle roots.

**Tasks:**
- Define a `RootOwnershipTransferred { listing_id, from, to }` event struct
- Emit it at the end of `transfer_root_ownership`
- Add a test asserting the event is emitted with correct values

---

## Issue #115: `zk_verifier` `ContractError` has only one variant (`Unauthorized = 1`) — all other error conditions produce unstructured panics

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 45 minutes

**Description:**
`zk_verifier` only defines `ContractError::Unauthorized`. Any other error condition (e.g. invalid proof path length, missing root during transfer) falls through to unstructured panics or returns `false`. This makes it impossible for callers to distinguish between "proof failed" and "contract misconfigured".

**Tasks:**
- Add `RootNotFound = 2`, `ProofTooLong = 3`, and `InvalidInput = 4` variants to `ContractError`
- Use `RootNotFound` in `transfer_root_ownership` when no owner is stored
- Use `ProofTooLong` in `verify_partial_proof` when path exceeds `MAX_PROOF_DEPTH`
- Update all affected tests

---

## Issue #116: `atomic_swap` `initialize` does not validate that `fee_bps <= 10_000`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`initialize` accepts any `u32` value for `fee_bps` with no upper-bound check. A `fee_bps` greater than 10,000 (100%) would cause `calculate_fee_amount` to return a fee larger than the swap amount, making `seller_amount` negative and causing an underflow panic in `release_to_seller`.

**Tasks:**
- Add `if fee_bps > 10_000 { panic_with_error!(&env, ContractError::InvalidAmount) }` in `initialize`
- Add a `FeeBpsTooHigh` variant to `ContractError` for a more descriptive error
- Add a test that passes `fee_bps = 10_001` to `initialize` and expects the error

---

## Issue #117: `atomic_swap` has no `update_config` function — fee and cancel delay cannot be changed after deployment

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
`Config` (fee_bps, fee_recipient, cancel_delay_secs, zk_verifier) is set once at `initialize` and cannot be changed. The admin has no way to adjust the protocol fee or update the trusted `zk_verifier` address without redeploying the contract.

**Tasks:**
- Add `update_config(env, fee_bps, fee_recipient, cancel_delay_secs)` gated by admin auth
- Validate `fee_bps <= 10_000`
- Emit a `ConfigUpdated` event
- Add tests for successful update and unauthorized attempt

---

## Issue #118: `atomic_swap` has no `transfer_admin` function — admin key cannot be rotated

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 45 minutes

**Description:**
The admin address is set at `initialize` and stored separately from `Config`. There is no function to transfer the admin role. If the admin key is compromised or needs rotation, there is no on-chain recovery path.

**Tasks:**
- Add `transfer_admin(env, new_admin: Address)` requiring current admin auth
- Update `DataKey::Admin` to the new address
- Emit the existing `AdminTransferred { old_admin, new_admin }` event
- Add tests for successful transfer and unauthorized attempt

---

## Issue #119: CI `test` job runs `cargo test --workspace` without `--locked`, allowing untested dependency drift

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** Medium

**Estimated Time:** 15 minutes

**Description:**
The `test` job in `.github/workflows/ci.yml` runs `cargo test --workspace` without the `--locked` flag. This means CI can pass even if `Cargo.lock` is out of date or if a dependency has been silently updated. The `scripts/test.sh` script correctly uses `--locked`, but CI does not call the script — it runs `cargo test` directly.

**Tasks:**
- Change the `Run tests` step in `ci.yml` to `cargo test --locked --workspace`
- Or change it to call `./scripts/test.sh` directly so CI and local runs are identical
- Add a comment in `ci.yml` explaining why `--locked` is required

---

## Issue #120: CI has no `clippy` or `rustfmt` lint job

**Labels:** `enhancement`

**Body:**

**Category:** DevOps - Enhancement

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
The CI pipeline only runs tests and a WASM build. There is no `cargo clippy` or `cargo fmt --check` step. Lint regressions and formatting inconsistencies are only caught locally if developers remember to run them.

**Tasks:**
- Add a `lint` job to `ci.yml` that runs `cargo clippy --workspace -- -D warnings`
- Add a `fmt` job (or step) that runs `cargo fmt --all -- --check`
- Ensure both jobs use the same Rust toolchain version as the `test` job

---

## Issue #121: `deploy_testnet.sh` `initialize` invocation is missing the `zk_verifier` argument

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** High

**Estimated Time:** 20 minutes

**Description:**
`deploy_testnet.sh` calls `stellar contract invoke ... initialize` with `--admin`, `--fee_bps`, `--fee_recipient`, and `--cancel_delay_secs`. The `atomic_swap` `initialize` function also requires `--zk_verifier` (the deployed `ZK_VERIFIER` contract address). The script deploys `ZK_VERIFIER` before calling `initialize`, so the address is available, but it is never passed. Every deployment will fail at the `initialize` step.

**Tasks:**
- Add `--zk_verifier "$ZK_VERIFIER"` to the `initialize` invocation in `deploy_testnet.sh`
- Add `ATOMIC_SWAP_ZK_VERIFIER` as an optional env var override (defaulting to the just-deployed `$ZK_VERIFIER`)
- Add a note in `.env.example` and `README.md` about this parameter

---

## Issue #122: `deploy_testnet.sh` does not initialize `ip_registry` after deployment

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** High

**Estimated Time:** 20 minutes

**Description:**
`deploy_testnet.sh` deploys all three contracts but only calls `initialize` on `atomic_swap`. `ip_registry` requires `initialize(admin, ttl_threshold, ttl_extend_to)` before any function can be called. Any call to `register_ip` after deployment will panic with `NotInitialized`.

**Tasks:**
- Add an `initialize` invocation for `ip_registry` after deployment, reading `ATOMIC_SWAP_ADMIN` as the admin
- Add `IP_REGISTRY_TTL_THRESHOLD` and `IP_REGISTRY_TTL_EXTEND_TO` env vars with sensible defaults
- Document these vars in `.env.example`

---

## Issue #123: `.env.example` duplicates `VITE_CONTRACT_ATOMIC_SWAP`, `VITE_CONTRACT_IP_REGISTRY`, and `VITE_CONTRACT_ZK_VERIFIER` — one set is always empty

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
`.env.example` defines `VITE_CONTRACT_ATOMIC_SWAP`, `VITE_CONTRACT_IP_REGISTRY`, and `VITE_CONTRACT_ZK_VERIFIER` twice — once in the "Contract addresses" section and again in the "Frontend configuration" section. Both sets are empty. This confuses contributors who fill in one set but not the other, leading to a broken frontend.

**Tasks:**
- Remove the duplicate `VITE_*` entries from the "Contract addresses" section
- Keep only the "Frontend configuration" section for `VITE_*` vars
- Add a comment: "Copy CONTRACT_* values above into VITE_CONTRACT_* after deployment"

---

## Issue #124: `README.md` references `docs/architecture.md` which does not exist

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Medium

**Estimated Time:** 1 hour

**Description:**
The `README.md` overview section links to `./docs/architecture.md` for sequence diagrams, but no `docs/` directory or `architecture.md` file exists in the repository. Clicking the link returns a 404 on GitHub.

**Tasks:**
- Create `docs/architecture.md` with a sequence diagram covering the full swap lifecycle (register → set root → initiate → confirm → release / dispute)
- Use Mermaid syntax so GitHub renders it inline
- Or remove the broken link from `README.md` if the doc is not planned

---

## Issue #125: `README.md` `Security` section links to `SECURITY.md` which was deleted

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`README.md` has a `## Security` section that links to `./SECURITY.md`. The `SECURITY.md` file was deleted in the latest pull. The link is broken and the security disclosure process is undocumented.

**Tasks:**
- Recreate `SECURITY.md` with a responsible disclosure policy (contact email or GitHub Security Advisories link)
- Or inline a brief security section directly in `README.md` and remove the broken link
- Ensure the file is referenced correctly from `README.md`

---

## Issue #126: `Makefile` `deploy-mainnet` target reuses `deploy_testnet.sh` with `STELLAR_NETWORK=mainnet` — script name is misleading and mainnet init args may differ

**Labels:** `documentation`

**Body:**

**Category:** Documentation / DevOps

**Priority:** Low

**Estimated Time:** 30 minutes

**Description:**
The `Makefile` `deploy-mainnet` target sets `STELLAR_NETWORK=mainnet` and calls `deploy_testnet.sh`. The script name implies testnet-only usage, and there is no documentation warning that mainnet deployments require a funded account, different RPC endpoints, and potentially different TTL and fee parameters.

**Tasks:**
- Rename `deploy_testnet.sh` to `deploy.sh` and update all references in `Makefile`, `README.md`, and `build.sh`
- Add a `--network` parameter to the script that defaults to `testnet`
- Add a prominent warning block in the script when `STELLAR_NETWORK=mainnet`
- Document mainnet-specific prerequisites in `README.md`

---

## Issue #127: `ip_registry` `IpRegistered` event does not include `price_usdc` or `royalty_bps`

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
The `IpRegistered` event only emits `listing_id`, `owner`, `ipfs_hash`, and `merkle_root`. Off-chain indexers building a listing feed cannot determine the price or royalty from the event alone — they must make a separate `get_listing` call per event.

**Tasks:**
- Add `price_usdc: i128` and `royalty_bps: u32` fields to `IpRegistered`
- Update `register_ip` to populate these fields when emitting the event
- Update the `BatchIpRegistered` event similarly
- Update tests that assert event data

---

## Issue #128: `atomic_swap` `SwapConfirmed` event exposes the raw `decryption_key` bytes on-chain

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`SwapConfirmed` includes `decryption_key: Bytes` as a non-topic field. This means the full decryption key is written into the public ledger event stream. Any observer can read the key from the event without being the buyer. The key should only be accessible to the buyer via `get_decryption_key`, not broadcast publicly.

**Tasks:**
- Remove `decryption_key` from the `SwapConfirmed` event struct
- Keep only `swap_id` and `seller` in the event
- Update the test `test_confirm_swap_valid_proof` to not assert on the key in the event
- Add a comment explaining that the key is intentionally omitted from the event

---

## Issue #129: `atomic_swap` `calculate_fee_amount` panics with `InvalidAmount` on overflow but `InvalidAmount` semantically means "zero or negative amount"

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
`calculate_fee_amount` uses `checked_mul(...).unwrap_or_else(|| env.panic_with_error(ContractError::InvalidAmount))` to handle overflow. `InvalidAmount` is documented as "zero or negative amount" but is reused here for arithmetic overflow, making error codes ambiguous for callers.

**Tasks:**
- Add an `Overflow = 18` variant to `ContractError`
- Replace the overflow `panic_with_error!(ContractError::InvalidAmount)` with `ContractError::Overflow`
- Add a test that triggers the overflow path and asserts `Overflow`

---

## Issue #130: `ip_registry` `initialize` returns `Result<(), ContractError>` but `atomic_swap` `initialize` panics — inconsistent error handling style across contracts

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 45 minutes

**Description:**
`ip_registry::initialize` returns `Result<(), ContractError>` using `return Err(ContractError::AlreadyInitialized)`. `atomic_swap::initialize` calls `env.panic_with_error(ContractError::AlreadyInitialized)` and returns `()`. `zk_verifier` has no `initialize` at all. This inconsistency means callers must use `try_initialize` for `ip_registry` but not for `atomic_swap`, and test code must handle both patterns.

**Tasks:**
- Standardise both contracts to use `panic_with_error!` (the Soroban idiomatic approach) instead of `Result` returns for `initialize`
- Update `ip_registry::initialize` to use `panic_with_error!` and return `()`
- Update all tests that use `try_initialize` on `ip_registry` to match on the panic

---

## Issue #131: `ip_registry` `update_listing` does not emit an event after updating

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`update_listing` silently modifies `ipfs_hash` and `merkle_root` in persistent storage with no on-chain event. Off-chain indexers cannot detect that a listing's content has changed without polling every listing on every block.

**Tasks:**
- Define an `IpUpdated { listing_id, owner, new_ipfs_hash, new_merkle_root }` event struct with `#[contractevent]`
- Emit it at the end of `update_listing`
- Add a test asserting the event is emitted with the correct new values

---

## Issue #132: `ip_registry` `list_by_owner` does not extend TTL on the `OwnerIndex` key when read

**Labels:** `enhancement`

**Body:**

**Category:** Smart Contract - Enhancement

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
`list_by_owner` reads `DataKey::OwnerIndex(owner)` from persistent storage without extending its TTL. An owner who only reads their index (without registering new listings) will eventually have their index expire, causing `list_by_owner` to return an empty vec even though their listings still exist.

**Tasks:**
- Call `extend_persistent` on `DataKey::OwnerIndex(owner)` inside `list_by_owner` when the entry exists
- Apply the same fix to `list_by_owner_page`
- Add a test that reads the owner index near TTL expiry and verifies it persists

---

## Issue #133: `ip_registry` `batch_register_ip` emits both individual `IpRegistered` events and a `BatchIpRegistered` event — double-event per entry

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
Inside the batch loop, `batch_register_ip` calls `IpRegistered { ... }.publish(&env)` for every entry, and then after the loop emits a single `BatchIpRegistered` event. This means a batch of N entries produces N+1 events. Indexers that listen for `IpRegistered` will double-count batch registrations.

**Tasks:**
- Remove the per-entry `IpRegistered` publish inside the batch loop
- Keep only the single `BatchIpRegistered` event after the loop
- Update the `test_batch_register_ip_emits_events` test to assert only one event is emitted

---

## Issue #134: `ip_registry` `deregister_listing` does not extend TTL on the `OwnerIndex` after removing the entry

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Low

**Estimated Time:** 20 minutes

**Description:**
`deregister_listing` removes the listing ID from the `OwnerIndex` vec and writes it back, but does not call `extend_persistent` on the index key. If the index is near expiry, the write will succeed but the TTL will not be refreshed, and the index may expire shortly after.

**Tasks:**
- Add `extend_persistent(&env, &idx_key, &cfg)` after writing the updated `OwnerIndex` in `deregister_listing`
- Add a test that deregisters a listing near TTL expiry and verifies the index persists

---

## Issue #135: `atomic_swap` `initiate_swap` does not validate that `seller != buyer`

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`initiate_swap` accepts any `seller` address including the same address as `buyer`. A user could initiate a swap with themselves, locking their own USDC in the contract and then confirming it to retrieve the decryption key without any real counterparty. This could be used to game the indexer or test the contract in unexpected ways.

**Tasks:**
- Add `if buyer == seller { panic_with_error!(&env, ContractError::SellerMismatch) }` at the start of `initiate_swap`
- Add a test that passes `buyer == seller` and expects `SellerMismatch`

---

## Issue #136: `atomic_swap` `confirm_swap` does not update `ActiveListingSwap` status after confirmation

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
After `confirm_swap` sets `swap.status = SwapStatus::Completed`, the `DataKey::ActiveListingSwap(listing_id)` entry still points to the now-completed swap. `is_listing_available` checks `swap.status != Pending`, so it correctly returns `true` after confirmation. However, a new `initiate_swap` on the same listing will find the stale `ActiveListingSwap` entry, load the completed swap, and pass the `status != Pending` check — allowing a second swap to be initiated without clearing the old entry first.

**Tasks:**
- Remove `DataKey::ActiveListingSwap(swap.listing_id)` from persistent storage at the end of `confirm_swap`
- Add a test that confirms a swap and then initiates a new swap on the same listing successfully

---

## Issue #137: `zk_verifier` `set_merkle_root` does not validate that `root` is non-zero

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`set_merkle_root` accepts any `BytesN<32>` including an all-zero root `[0u8; 32]`. A zero root is trivially forgeable — any proof path that produces an all-zero hash would verify successfully. Sellers could accidentally or maliciously set a zero root.

**Tasks:**
- Add a check that rejects `root == BytesN::from_array(&env, &[0u8; 32])` with `ContractError::Unauthorized` or a new `InvalidRoot` variant
- Add a test that calls `set_merkle_root` with a zero root and expects the error

---

## Issue #138: `atomic_swap` `Swap` struct field `usdc_token` is never validated against `Config.allowed_tokens` at initiation

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** High

**Estimated Time:** 1 hour

**Description:**
`initiate_swap` accepts any `usdc_token: Address` and stores it in the `Swap` struct. There is no `allowed_tokens` list in `Config` and no validation that the token is a known USDC contract. A buyer could pass any token contract (including one they control), effectively paying with worthless tokens.

**Tasks:**
- Add `allowed_tokens: Vec<Address>` to `Config` and set it in `initialize`
- In `initiate_swap`, check that `usdc_token` is in `config.allowed_tokens` and panic with a new `InvalidToken` error if not
- Update all tests and the `setup_full` helper to pass `allowed_tokens`

---

## Issue #139: `ip_registry` `transfer_listing_ownership` does not update `royalty_recipient` when ownership changes

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
`transfer_listing_ownership` updates `listing.owner` to `new_owner` but leaves `listing.royalty_recipient` pointing to the old owner. After a transfer, royalties will still be paid to the previous owner unless the new owner explicitly calls `update_listing` to change the recipient. This is a silent fund-routing bug.

**Tasks:**
- After updating `listing.owner = new_owner.clone()`, also set `listing.royalty_recipient = new_owner.clone()` (or accept an optional `new_royalty_recipient` parameter)
- Add a test that transfers ownership and verifies royalties are paid to the new owner after a swap

---

## Issue #140: `atomic_swap` `release_to_seller` does not read royalty data from `ip_registry` — royalties are never paid

**Labels:** `bug`

**Body:**

**Category:** Smart Contract - Bug

**Priority:** Critical

**Estimated Time:** 2 hours

**Description:**
`release_to_seller` deducts the protocol fee and sends the remainder to `swap.seller`, but never calls `IpRegistryClient::get_listing` to read `royalty_bps` and `royalty_recipient`. The `Listing` struct has these fields, but they are completely ignored at settlement time. IP creators receive no royalties.

**Tasks:**
- In `release_to_seller`, call `IpRegistryClient::new(&env, &config.ip_registry).get_listing(&swap.listing_id)` to fetch royalty data
- Deduct `royalty_amount = amount * royalty_bps / 10_000` from `seller_amount` and transfer it to `royalty_recipient`
- Handle the case where `get_listing` returns `None` (listing was deregistered) by skipping royalty payment
- Add a test asserting the royalty recipient receives the correct amount

---

## Issue #141: `frontend/.env.example` contains hardcoded testnet contract addresses that will be invalid for other deployers

**Labels:** `bug`, `documentation`

**Body:**

**Category:** Frontend - Bug / Documentation

**Priority:** Medium

**Estimated Time:** 15 minutes

**Description:**
`frontend/.env.example` has hardcoded values for `VITE_CONTRACT_ATOMIC_SWAP`, `VITE_CONTRACT_IP_REGISTRY`, and `VITE_CONTRACT_ZK_VERIFIER` pointing to specific testnet contract IDs. Any developer who copies this file without replacing the values will connect to someone else's contracts. The root `.env.example` correctly leaves these blank.

**Tasks:**
- Replace the hardcoded contract IDs in `frontend/.env.example` with empty placeholders and a comment explaining they must be filled after deployment
- Add a comment pointing to `./scripts/deploy_testnet.sh` as the source of these values

---

## Issue #142: `README.md` CI badge points to `unixfundz/Atomic-IP-Marketplace` instead of `MerkleMint/Atomic-IP-Marketplace`

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Low

**Estimated Time:** 5 minutes

**Description:**
The CI badge URL in `README.md` references `https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml/badge.svg` and links to `unixfundz/Atomic-IP-Marketplace`. This is the wrong repository and the badge will show the wrong CI status (or fail to load).

**Tasks:**
- Update the badge URL and link to point to `MerkleMint/Atomic-IP-Marketplace`

---

## Issue #143: `README.md` `Security` section links to `SECURITY.md` which was deleted

**Labels:** `documentation`

**Body:**

**Category:** Documentation

**Priority:** Low

**Estimated Time:** 15 minutes

**Description:**
`README.md` contains `## Security\n[SECURITY.md](./SECURITY.md)` but `SECURITY.md` was deleted in the latest pull. The link is broken.

**Tasks:**
- Either recreate a minimal `SECURITY.md` with a responsible disclosure policy
- Or replace the section with inline security contact information
- Remove the broken link

---

## Issue #144: `deploy_testnet.sh` does not initialize `ip_registry` after deployment

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** High

**Estimated Time:** 30 minutes

**Description:**
`deploy_testnet.sh` deploys all three contracts and initializes `atomic_swap`, but never calls `initialize` on `ip_registry`. Any call to `register_ip` or `get_listing` after deployment will panic with `ContractError::NotInitialized` because `Config` is never stored.

**Tasks:**
- Add a `stellar contract invoke` call for `ip_registry initialize` after deployment, passing `admin`, `ttl_threshold`, and `ttl_extend_to` from `.env` variables
- Add `IP_REGISTRY_ADMIN`, `IP_REGISTRY_TTL_THRESHOLD`, and `IP_REGISTRY_TTL_EXTEND_TO` to `.env.example`
- Add a smoke test that calls `ip_registry get_config` after initialization

---

## Issue #145: `deploy_testnet.sh` `initialize` call for `atomic_swap` is missing the `zk_verifier` argument

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** High

**Estimated Time:** 20 minutes

**Description:**
The `atomic_swap initialize` invocation in `deploy_testnet.sh` passes `--admin`, `--fee_bps`, `--fee_recipient`, and `--cancel_delay_secs` but omits `--zk_verifier`. The `Config` struct requires a `zk_verifier: Address` field, so the initialization will fail with a missing argument error.

**Tasks:**
- Add `--zk_verifier "$ZK_VERIFIER"` to the `atomic_swap initialize` invocation (using the address returned by the `zk_verifier` deploy step)
- Verify the full deploy script runs end-to-end on testnet without errors

---

## Issue #146: CI `test` job does not use `--locked` flag — dependency drift is untested

**Labels:** `bug`

**Body:**

**Category:** DevOps - Bug

**Priority:** Medium

**Estimated Time:** 15 minutes

**Description:**
The `test` job in `.github/workflows/ci.yml` runs `cargo test --workspace` without `--locked`. This means CI can silently use different dependency versions than what is committed in `Cargo.lock`, making builds non-reproducible. The `scripts/test.sh` correctly uses `--locked` but CI does not call that script.

**Tasks:**
- Change `run: cargo test --workspace` to `run: cargo test --locked --workspace` in `ci.yml`
- Or change the step to `run: ./scripts/test.sh` to reuse the existing script

---

## Issue #147: CI has no `clippy` lint job — code quality regressions go undetected

**Labels:** `enhancement`

**Body:**

**Category:** DevOps - Enhancement

**Priority:** Medium

**Estimated Time:** 30 minutes

**Description:**
The CI pipeline has `test` and `build-wasm` jobs but no Clippy lint step. Clippy catches common Rust mistakes (unused variables, redundant clones, incorrect error handling patterns) that unit tests do not cover.

**Tasks:**
- Add a `lint` job to `ci.yml` that runs `cargo clippy --workspace -- -D warnings`
- Add a `rustfmt` check step: `cargo fmt --all -- --check`
- Run both on every push and pull request

---

## Issue #148: `SwapCard` displays `swap.usdc_amount` as a raw integer without USDC decimal conversion

**Labels:** `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** Medium

**Estimated Time:** 20 minutes

**Description:**
`SwapCard.tsx` renders `{swap.usdc_amount} USDC` directly. Since Stellar USDC uses 7 decimal places, a `usdc_amount` of `10_000_000` will display as `10000000 USDC` instead of `10.00 USDC`. `ConfirmSwapForm` correctly divides by `Math.pow(10, 7)` but `SwapCard` does not.

**Tasks:**
- Add a `USDC_DECIMALS = 7` constant to `SwapCard.tsx`
- Display `(swap.usdc_amount / Math.pow(10, USDC_DECIMALS)).toFixed(2)` instead of the raw value
- Apply the same fix to any other component that renders `usdc_amount` directly

---

## Issue #149: `ListingCard` displays `listing.price_usdc` divided by `1_000_000` (6 decimals) instead of `10_000_000` (7 decimals)

**Labels:** `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** Medium

**Estimated Time:** 15 minutes

**Description:**
`ListingCard.jsx` renders `{listing.price_usdc / 1_000_000} USDC`. Stellar USDC uses 7 decimal places (`10_000_000` stroops = 1 USDC), not 6. This means all displayed prices are 10× too high. `InitiateSwapModal` correctly uses `USDC_DECIMALS = 7`.

**Tasks:**
- Change `listing.price_usdc / 1_000_000` to `listing.price_usdc / 10_000_000` in `ListingCard.jsx`
- Or import a shared `USDC_DECIMALS` constant from a central config file
- Add a comment explaining the decimal convention

---

## Issue #150: `index.html` loads both `app.js` (vanilla JS) and `src/main.tsx` (React) — duplicate contract calls and conflicting DOM manipulation

**Labels:** `bug`

**Body:**

**Category:** Frontend - Bug

**Priority:** High

**Estimated Time:** 2 hours

**Description:**
`index.html` includes `<script type="module" src="app.js">` and `<script type="module" src="src/main.tsx">`. `app.js` is a legacy vanilla JS file that directly manipulates the DOM (populating `#listingsGrid`, handling the initiate swap modal) while `src/main.tsx` mounts React components into `#wallet-root`, `#dashboard-root`, and `#listings-dashboard-root`. Both scripts make independent contract calls, causing duplicate RPC requests and potential race conditions on shared DOM nodes.

**Tasks:**
- Migrate the listings grid and initiate swap modal from `app.js` into React components
- Remove `app.js` once all functionality is covered by React
- Ensure `index.html` only loads `src/main.tsx`

---
