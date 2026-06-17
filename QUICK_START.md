# Swap Status Dashboard - Quick Start

## 1. Install & Build

```bash
cd frontend
npm install
npm run typecheck  # Verify no errors
npm run build      # Build for production
```

## 2. Add to Your App

```tsx
// src/App.tsx
import { SwapStatusDashboard } from "./components/SwapStatusDashboard";
import { WalletProvider } from "./context/WalletContext";

export function App() {
  return (
    <WalletProvider>
      <SwapStatusDashboard />
    </WalletProvider>
  );
}
```

## 3. Configure Environment

```bash
# .env
VITE_CONTRACT_ATOMIC_SWAP=CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4
VITE_STELLAR_RPC_URL=https://soroban-testnet.stellar.org
VITE_STELLAR_NETWORK=testnet
```

## 4. Start Development

```bash
npm run dev
# Open http://localhost:5173
# Connect your wallet
# Dashboard will show pending swaps with real-time updates
```

## 5. Run Tests

```bash
# Unit tests
npm test -- SwapStatusDashboard.test.tsx

# Integration tests
npm test -- SwapStatusDashboard.integration.test.tsx
```

## File Locations

```
Components:
  frontend/src/components/SwapStatusDashboard.tsx
  frontend/src/components/SwapStatusBadge.tsx
  frontend/src/components/DisputeCountdown.tsx
  frontend/src/components/SwapHistoryTable.tsx

Service:
  frontend/src/services/contractPollingService.ts

Tests:
  frontend/tests/SwapStatusDashboard.test.tsx
  frontend/tests/SwapStatusDashboard.integration.test.tsx
  frontend/tests/mocks/contractMocks.ts

Docs:
  frontend/SWAP_DASHBOARD_GUIDE.md
  frontend/INTEGRATION_EXAMPLE.md
  IMPLEMENTATION_SUMMARY.md
  DASHBOARD_CHECKLIST.md
```

## Key Features

✅ Real-time polling (5-10s with exponential backoff)
✅ Status badges with animations
✅ Dispute countdown timers
✅ Transaction history with sorting/filtering
✅ WCAG 2.1 AA accessibility
✅ Keyboard navigation
✅ Dark mode support
✅ Mobile responsive
✅ Network error recovery
✅ Tab visibility awareness

## Component Usage

### SwapStatusDashboard
```tsx
<SwapStatusDashboard />
// No props needed - uses WalletContext and useMySwaps hook
```

### SwapStatusBadge
```tsx
<SwapStatusBadge 
  status="Pending" 
  confirmations={2}
  targetConfirmations={3}
/>
```

### DisputeCountdown
```tsx
<DisputeCountdown 
  expiresAt={1719216000}
  currentTime={1719198000}
  onExpired={() => console.log("Expired!")}
/>
```

### SwapHistoryTable
```tsx
<SwapHistoryTable 
  swaps={swaps}
  pageSize={15}
  currentPage={1}
/>
```

## Troubleshooting

### Polling not working?
- Check `.env` has contract ID
- Verify RPC URL is accessible
- Check browser console for errors

### Status not updating?
- Ensure wallet is connected
- Check pending swaps exist
- Wait 5-10 seconds for first poll
- Check network tab (F12)

### Styling issues?
- Clear browser cache
- Restart dev server
- Verify CSS files imported
- Check for conflicting styles

### Accessibility issues?
- Test with keyboard (Tab/Enter)
- Use screen reader (NVDA/JAWS)
- Check color contrast (axe DevTools)
- Test with reduced motion enabled

## Performance Tips

- Only pending swaps are polled
- Polling pauses when tab is hidden
- Completed swaps don't refresh
- Smart retry with exponential backoff
- Cleanup prevents memory leaks

## Need Help?

1. Read [SWAP_DASHBOARD_GUIDE.md](./frontend/SWAP_DASHBOARD_GUIDE.md)
2. Check [INTEGRATION_EXAMPLE.md](./frontend/INTEGRATION_EXAMPLE.md)
3. Review test files for examples
4. Check console for error messages

## Production Checklist

- [ ] Update `.env` with production contract IDs
- [ ] Test on testnet first
- [ ] Verify wallet connection works
- [ ] Test keyboard navigation
- [ ] Test with screen reader
- [ ] Test on mobile devices
- [ ] Check dark mode rendering
- [ ] Monitor polling performance
- [ ] Verify error handling
- [ ] Deploy to production

---

**Status: Ready to integrate** ✅
