# Swap Status Dashboard - Integration Example

## Quick Start

### 1. Import and render the component

```tsx
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

function App() {
  return (
    <main>
      <SwapStatusDashboard />
    </main>
  );
}
```

### 2. Ensure wallet context is available

```tsx
import { WalletProvider } from "./context/WalletContext";
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

export function App() {
  return (
    <WalletProvider>
      <SwapStatusDashboard />
    </WalletProvider>
  );
}
```

### 3. Configure environment variables

Add to `.env` (copy from `.env.example`):

```
VITE_CONTRACT_ATOMIC_SWAP=CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4
VITE_STELLAR_RPC_URL=https://soroban-testnet.stellar.org
VITE_STELLAR_NETWORK=testnet
```

## Component Features

### Real-time Status Updates

The dashboard automatically polls contract state every 5-10 seconds with exponential backoff for error recovery.

**Automatic features:**
- ✅ Polls only pending swaps (completed swaps are not polled)
- ✅ Pauses polling when tab is hidden
- ✅ Resumes polling when tab becomes visible
- ✅ Handles network errors gracefully with exponential backoff
- ✅ Shows connection errors to the user
- ✅ Cleanup on component unmount

### Status Badges

Display swap status with real-time confirmation progress:

```tsx
<SwapStatusBadge 
  status="Pending"
  confirmations={2}
  targetConfirmations={3}
/>
```

**Status types:**
- `Pending` - Yellow, with confirmation progress bar
- `Completed` - Green checkmark
- `Cancelled` - Red X
- `Disputed` - Red warning with animation

### Dispute Countdown

Shows time remaining before a disputed swap expires:

```tsx
<DisputeCountdown 
  expiresAt={1719216000}
  currentTime={1719198000}
  onExpired={() => handleExpired()}
/>
```

**States:**
- Normal (> 1h) - Yellow background
- Urgent (< 1h) - Orange background
- Critical (< 10m) - Red with pulse animation

### Transaction History

Sortable, filterable transaction table with pagination:

```tsx
<SwapHistoryTable 
  swaps={swaps}
  pageSize={15}
  currentPage={1}
/>
```

**Features:**
- Sort by ID, Date, Status, Amount
- Filter: All / Active / Resolved
- Pagination with configurable page size
- Keyboard accessible
- Screen reader friendly

## Usage in Different Scenarios

### Scenario 1: Integrated in main dashboard

```tsx
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

export function MainDashboard() {
  return (
    <div className="dashboard">
      <header>Atomic IP Marketplace</header>
      <SwapStatusDashboard />
      <footer>© 2024 MerkleMint</footer>
    </div>
  );
}
```

### Scenario 2: Dedicated page route

```tsx
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/swaps" element={<SwapStatusDashboard />} />
      </Routes>
    </BrowserRouter>
  );
}
```

### Scenario 3: Modal overlay

```tsx
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

export function App() {
  const [showDashboard, setShowDashboard] = useState(false);

  return (
    <>
      <button onClick={() => setShowDashboard(true)}>View Swaps</button>
      {showDashboard && (
        <dialog open>
          <SwapStatusDashboard />
          <button onClick={() => setShowDashboard(false)}>Close</button>
        </dialog>
      )}
    </>
  );
}
```

## Testing

### Run unit tests

```bash
npm test -- SwapStatusDashboard.test.tsx
```

Tests cover:
- Component rendering states
- Polling behavior
- Error handling
- Accessibility compliance
- Keyboard navigation

### Run integration tests

```bash
npm test -- SwapStatusDashboard.integration.test.tsx
```

Tests cover:
- Real-time status updates
- Exponential backoff retry logic
- Network error recovery
- Transaction history operations
- Performance optimization

### Using mocks in your own tests

```typescript
import {
  createMockSwap,
  mockGetSwapWithDelay,
  mockTransactionHistory,
} from "./mocks/contractMocks";

// Create test data
const swap = createMockSwap({ 
  status: "Pending",
  usdc_amount: 5000000000 
});

// Simulate contract responses
jest.mock("../lib/contractClient");
(contractClient.getSwap as jest.Mock).mockImplementation(() =>
  mockGetSwapWithDelay(1, "Completed", 50)
);

// Test your code
render(<SwapStatusDashboard />);
```

## Styling & Customization

### Responsive breakpoints

The dashboard is responsive out of the box:
- **Desktop** (≥769px) - Full grid layout
- **Tablet** (640-768px) - Adjusted spacing
- **Mobile** (<640px) - Single column

### Dark mode

Automatically respects `prefers-color-scheme`:

```css
@media (prefers-color-scheme: dark) {
  /* Dark mode styles automatically applied */
}
```

### High contrast mode

Enhanced visuals for `prefers-contrast: more`:

```css
@media (prefers-contrast: more) {
  /* Bolder borders and higher contrast */
}
```

### Reduced motion

Animations disabled when `prefers-reduced-motion: reduce`:

```css
@media (prefers-reduced-motion: reduce) {
  /* No animations */
}
```

## Accessibility

### WCAG 2.1 AA Compliance

✅ **Keyboard Navigation**
```
Tab      → Move to next interactive element
Shift+Tab → Move to previous interactive element
Enter    → Activate button
Space    → Toggle radio/checkbox
```

✅ **Screen Reader Support**
- Semantic HTML structure
- ARIA labels and roles
- Live regions for updates
- Descriptive button labels

✅ **Visual Accessibility**
- 2px focus visible outlines
- High contrast color combinations
- Support for prefers-reduced-motion
- Readable font sizes (14px minimum)

### Testing accessibility

```bash
# With axe DevTools browser extension
# 1. Open DevTools (F12)
# 2. Select Scan ALL of my page
# 3. Check results

# With screen reader (Windows: NVDA, macOS: VoiceOver)
# Tab through the dashboard and verify announcements

# Keyboard-only navigation
# Close mouse, navigate using Tab/Enter/Space/Arrow keys
```

## Performance

### Polling optimization

- **Only pending swaps polled** - Completed/cancelled swaps are skipped
- **Exponential backoff** - Reduces RPC load on errors
- **Tab visibility detection** - Pauses polling when hidden
- **Smart cleanup** - Stops polling on unmount

### Network usage

- 5-10 second polling interval (configurable)
- Single poll per swap (no duplicates)
- ~1-2 KB per response
- Roughly 360-720 KB/day for 10 active swaps

## Troubleshooting

### Dashboard shows "Connect your wallet"

**Issue:** Wallet not connected
**Solution:** Ensure wallet context is properly set up and user has connected wallet

### Status not updating

**Issue:** Polling not working
**Solutions:**
1. Check `.env` has `VITE_CONTRACT_ATOMIC_SWAP` set
2. Verify `VITE_STELLAR_RPC_URL` is accessible
3. Open browser console (F12) for error messages
4. Check network tab - should see requests every 5-10s

### Animations stuttering

**Issue:** Performance problem
**Solutions:**
1. Check browser's developer tools for CPU/memory issues
2. Reduce polling frequency in contractPollingService
3. Disable animations via CSS variables
4. Close other tabs/applications

### Screen reader not announcing updates

**Issue:** Accessibility problem
**Solutions:**
1. Check ARIA labels are present in HTML
2. Verify live regions have `aria-live="polite"`
3. Test with multiple screen readers (NVDA, JAWS, VoiceOver)

## API Reference

### SwapStatusDashboard

```tsx
<SwapStatusDashboard />
```

No props required. Uses context for wallet and swap data.

### SwapStatusBadge

```tsx
<SwapStatusBadge 
  status="Pending"           // Required
  confirmations={2}          // Optional
  targetConfirmations={3}    // Optional (default: 3)
/>
```

### DisputeCountdown

```tsx
<DisputeCountdown 
  expiresAt={1719216000}     // Required: Unix timestamp
  currentTime={1719198000}   // Required: Unix timestamp
  onExpired={() => {}}       // Optional: Callback
/>
```

### SwapHistoryTable

```tsx
<SwapHistoryTable 
  swaps={[]}                 // Required: Swap[]
  currentPage={1}            // Optional (default: 1)
  pageSize={10}              // Optional (default: 10)
/>
```

### ContractPollingService

```typescript
import { pollingService } from "./services/contractPollingService";

// Start polling
pollingService.poll(
  "swap_1",                  // Unique ID
  () => getSwap(1),          // Fetch function
  (data) => {},              // On success
  (error) => {}              // On error
);

// Stop specific poll
pollingService.stop("swap_1");

// Stop all polls
pollingService.stopAll();

// Resume all polls
pollingService.resume();
```

## Production Deployment

### Environment variables

```bash
# Testnet
VITE_STELLAR_NETWORK=testnet
VITE_STELLAR_RPC_URL=https://soroban-testnet.stellar.org
VITE_CONTRACT_ATOMIC_SWAP=<testnet_contract_id>

# Mainnet
VITE_STELLAR_NETWORK=mainnet
VITE_STELLAR_RPC_URL=https://soroban-mainnet.stellar.org
VITE_CONTRACT_ATOMIC_SWAP=<mainnet_contract_id>
```

### Build optimizations

```bash
# Build with optimizations
npm run build

# Check bundle size
npm run build -- --outDir dist --analyze
```

### Monitoring

Monitor these metrics in production:
- **Polling latency** - Track response times
- **Error rate** - Monitor failed requests
- **Connection errors** - Track user impact
- **Polling duration** - Watch for slow polls

## Future Enhancements

- [ ] WebSocket support for push updates
- [ ] Batch polling for multiple swaps
- [ ] Transaction detail modal
- [ ] Export history (CSV/JSON)
- [ ] Advanced filtering (date range, amount)
- [ ] Transaction search
- [ ] Dispute resolution UI
- [ ] Block confirmation tracking
- [ ] Fee estimation display
- [ ] Transaction replay/retry

## Support

For issues or questions:
1. Check [SWAP_DASHBOARD_GUIDE.md](./SWAP_DASHBOARD_GUIDE.md)
2. Review console errors (F12 → Console)
3. Check network requests (F12 → Network)
4. Open an issue with:
   - Browser and version
   - Steps to reproduce
   - Error messages
   - Expected vs actual behavior
