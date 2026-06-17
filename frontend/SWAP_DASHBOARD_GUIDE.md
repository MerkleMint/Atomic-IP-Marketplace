# Swap Status Dashboard Implementation Guide

## Overview

The Swap Status Dashboard provides real-time transaction tracking with:
- **Live polling** with exponential backoff (5-30s intervals)
- **Real-time status badges** with blockchain confirmation tracking
- **Dispute countdown timers** with urgency indicators
- **Sortable transaction history** with filtering
- **WCAG 2.1 AA accessibility** and keyboard navigation
- **Graceful error handling** with connection recovery

## Components

### SwapStatusDashboard.tsx
Main dashboard component orchestrating polling, UI updates, and visibility handling.

**Features:**
- Tab-based view switching (Active/History)
- Real-time polling for pending swaps
- Automatic pause/resume on tab visibility change
- Error boundary and connection status display

**Props:** None (uses context)

```tsx
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

export function App() {
  return <SwapStatusDashboard />;
}
```

### SwapStatusBadge.tsx
Displays status with animations and confirmation progress.

**Props:**
- `status: "Pending" | "Completed" | "Cancelled" | "Disputed"`
- `confirmations?: number` - Current block confirmations
- `targetConfirmations?: number` - Target for full confirmation (default: 3)

```tsx
<SwapStatusBadge 
  status="Pending" 
  confirmations={2} 
  targetConfirmations={3}
/>
```

### DisputeCountdown.tsx
Countdown timer with urgency indicators.

**Props:**
- `expiresAt: number` - Unix timestamp of expiration
- `currentTime: number` - Current ledger timestamp
- `onExpired?: () => void` - Callback when countdown reaches zero

**Visual states:**
- Normal (> 1 hour): Yellow background
- Urgent (< 1 hour): Orange background with animation
- Critical (< 10 minutes): Red background with pulse

```tsx
<DisputeCountdown 
  expiresAt={1719216000} 
  currentTime={1719198000}
  onExpired={() => console.log("Expired!")}
/>
```

### SwapHistoryTable.tsx
Sortable, filterable transaction history with pagination.

**Props:**
- `swaps: Swap[]` - Array of swap transactions
- `ledgerTimestamp: number` - Current ledger time
- `currentPage?: number` - Current pagination page (default: 1)
- `pageSize?: number` - Items per page (default: 10)

**Filters:**
- All transactions
- Active (Pending)
- Resolved (Completed/Cancelled)

**Sortable columns:**
- ID
- Created Date
- Status
- USDC Amount

```tsx
<SwapHistoryTable 
  swaps={swaps}
  ledgerTimestamp={ledgerTimestamp}
  pageSize={15}
  currentPage={1}
/>
```

## Services

### ContractPollingService
Handles polling with exponential backoff and visibility-aware pause/resume.

**Configuration:**
```typescript
{
  minInterval: 5000,        // 5 seconds
  maxInterval: 30000,       // 30 seconds
  backoffMultiplier: 1.5,   // Multiply interval on error
  maxRetries: 3             // Retries before error
}
```

**Usage:**
```typescript
import { pollingService } from "./services/contractPollingService";

// Start polling
pollingService.poll(
  "swap_1",                    // Unique ID
  () => getSwap(1),            // Fetch function
  (data) => updateUI(data),    // On success
  (error) => handleError(error) // On error
);

// Stop specific poll
pollingService.stop("swap_1");

// Stop all and pause on tab hide
pollingService.stopAll();

// Resume all on tab show
pollingService.resume();
```

## Integration

### Adding to your app

1. **Place component in routing:**
```tsx
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";

function App() {
  return (
    <div>
      <Header />
      <SwapStatusDashboard />
      <Footer />
    </div>
  );
}
```

2. **Ensure wallet context is available:**
```tsx
import { WalletProvider } from "./context/WalletContext";

function App() {
  return (
    <WalletProvider>
      <SwapStatusDashboard />
    </WalletProvider>
  );
}
```

3. **Contract client must be configured:**
The component uses `getSwap()` from `lib/contractClient.ts`. Ensure:
- `CONTRACT_ATOMIC_SWAP` env var is set
- `STELLAR_RPC_URL` is configured
- Contract methods return properly formatted Swap objects

## Testing

### Unit Tests
Located in `__tests__/SwapStatusDashboard.test.ts`

```bash
npm test -- SwapStatusDashboard.test.ts
```

**Covered:**
- Wallet connection states
- Loading and error states
- Tab switching
- Keyboard navigation
- Screen reader accessibility

### Integration Tests with Mocked Responses
Located in `__tests__/SwapStatusDashboard.integration.test.ts`

```bash
npm test -- SwapStatusDashboard.integration.test.ts
```

**Covered:**
- Real-time status updates
- Exponential backoff retry logic
- Network error handling
- Transaction history filtering/sorting
- Dispute countdown behavior
- Performance optimization

### Mock Utilities
Use `__tests__/mocks/contractMocks.ts` for test setup:

```typescript
import {
  createMockSwap,
  mockGetSwapWithDelay,
  mockNetworkError,
  createStatusProgression,
  mockTransactionHistory,
} from "./mocks/contractMocks";

// Create mock swap with overrides
const swap = createMockSwap({ 
  status: "Pending",
  usdc_amount: 5000000000
});

// Simulate delayed response
await mockGetSwapWithDelay(1, "Completed", 100);

// Simulate network error
mockNetworkError("Network timeout");

// Track status progression through updates
const progression = createStatusProgression(1);
progression.getNext(); // "Pending"
progression.getNext(); // "Pending"
progression.getNext(); // "Completed"
```

## Performance Optimization

### Smart Polling
- **Only polls pending swaps** - Completed/Cancelled swaps don't poll
- **Exponential backoff** - Reduces load on RPC on errors
- **Tab visibility detection** - Pauses polling when tab hidden
- **Automatic cleanup** - Stops polling when component unmounts

### Caching Strategies
- Uses `useMySwaps` hook for deduplicated swaps fetching
- Polling only updates individual swap objects
- Transaction history cached from initial `useMySwaps` call

### Network Optimization
- 5-second minimum polling interval (configurable)
- 30-second maximum interval on persistent errors
- Single polling job per swap (no duplicates)
- Visibility API to prevent background polling

## Accessibility

### WCAG 2.1 AA Compliance
✓ **Keyboard Navigation**
- Tab through all interactive elements
- Enter/Space to activate buttons
- Arrow keys in tables (where applicable)

✓ **Screen Reader Support**
- Semantic HTML (`<section>`, `<article>`, `<table>`)
- ARIA labels and roles (`aria-label`, `role="tab"`)
- Live regions for status updates (`aria-live="polite"`)
- Proper heading hierarchy

✓ **Visual Indicators**
- Status badges with contrasting colors
- Focus visible outlines (2px solid)
- Animations respect `prefers-reduced-motion`
- High contrast mode support

✓ **Responsive Design**
- Mobile-first approach
- Readable at all viewport sizes
- Touch-friendly button sizing
- Proper font sizing for readability

## Styling Customization

### CSS Variables (recommended)
```css
--ssd-primary: hsl(210, 100%, 50%);
--ssd-pending: hsl(38, 100%, 90%);
--ssd-completed: hsl(142, 71%, 90%);
--ssd-error: hsl(0, 84%, 90%);
--ssd-spacing: 1rem;
--ssd-radius: 0.5rem;
```

### Theme Integration
All components support light/dark mode via `prefers-color-scheme`:

```css
@media (prefers-color-scheme: dark) {
  /* Dark mode styles automatically applied */
}
```

## Troubleshooting

### Polling not updating
1. Check `CONTRACT_ATOMIC_SWAP` is set in `.env`
2. Verify RPC endpoint connectivity
3. Check browser console for errors
4. Ensure `getSwap()` returns proper Swap objects

### Status badges not animating
1. Check CSS files are imported
2. Verify `prefers-reduced-motion` isn't globally disabled
3. Check for CSS conflicts with other stylesheets

### Table not showing history
1. Ensure `useMySwaps` hook is populated
2. Check filters are not too restrictive
3. Verify ledger timestamp is current

### Accessibility issues
1. Run axe DevTools scan
2. Test with keyboard (Tab/Enter/Shift+Tab)
3. Test with screen reader (NVDA/JAWS)
4. Check Windows High Contrast mode

## Future Enhancements

- WebSocket support for push updates (instead of polling)
- Real-time blockchain confirmation tracking
- Batch polling for multiple swaps
- Transaction detail modal
- Export transaction history (CSV/JSON)
- Advanced filtering (date range, amount range)
- Transaction search by ID or party
- Dispute resolution UI integration

## Related Files

- Component: `frontend/src/components/SwapStatusDashboard.tsx`
- Service: `frontend/src/services/contractPollingService.ts`
- Hooks: `frontend/src/hooks/useMySwaps.ts`
- Tests: `frontend/src/components/__tests__/*.test.ts`
- Styling: `frontend/src/components/*.css`
