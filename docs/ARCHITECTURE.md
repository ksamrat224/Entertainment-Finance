# Runway architecture

Runway uses MagicBlock ERs for repeated, low-latency race writes and Solana devnet for durable settlement. It never takes custody or executes a trade.

| Account | Persistence | Runtime role | Lifecycle |
| --- | --- | --- | --- |
| `UserProfile` | Solana | Durable streak history | Base only |
| `DailyRun` | Solana, temporarily delegated | Public score and terminal race status | Base → ER → commit/undelegate → base |
| `SessionSponsor` | Solana, temporarily delegated | Funds ER-only account rent | Base → ER → undelegate → close/refund |
| `LiveRun` | ER-only ephemeral | Check-in event detail and score progression | Create/use/close in ER only |
| `AllocationReceipt` / `Achievement` | Solana | Immutable mock allocation and badge | Wallet-only base settlement |

## Routing

1. Initialize and delegate on `NEXT_PUBLIC_SOLANA_RPC`.
2. Call `getDelegationStatus` at `NEXT_PUBLIC_MAGICBLOCK_ROUTER` and use its FQDN for ER race operations.
3. Commit-and-undelegate on the returned ER endpoint.
4. Confirm the base account state before asking the wallet to settle the allocation receipt.

The Oracle is display-only. The client validates its expected feed ID, numeric domain, and freshness; the program does not consume prices or transfer assets.

