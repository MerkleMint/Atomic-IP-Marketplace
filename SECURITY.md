# Security Policy

## Responsible Disclosure

We take security seriously and appreciate your efforts to protect our users and the integrity of Atomic IP Marketplace contracts.

If you discover a vulnerability, we would like to know about it as soon as possible. Please report it privately to the project maintainers via email at `farouq@atomic-ip-marketplace.com` (or your preferred contact).

### Disclosure Process
1. **Report the issue** privately with a detailed description, including reproduction steps, impact, and any proof-of-concept.
2. We will acknowledge receipt within **48 hours**.
3. We will assess and work on a fix with **7 days** target resolution for critical issues.
4. Do not publicly disclose the vulnerability without our explicit permission.
5. We may credit responsible reporters in release notes.

Preferred reporting format:
- Use [GPG encryption](https://emailselfdefense.fsf.org/) if possible.
- Include contract affected (e.g., atomic_swap), network (testnet/mainnet), and version.

## Scope
This policy applies to:
- Soroban contracts: `atomic_swap`, `ip_registry`, `zk_verifier`.
- Deployment scripts (`deploy_testnet.sh`).
- Related infrastructure under our control.

## Known Limitations
- **Smart Contract Risks**: While Soroban provides protections (e.g., no reentrancy), risks like economic exploits, oracle manipulation (if used), or pause mechanism abuse possible. No formal security audit conducted yet.
- **Testnet Focus**: Current deploys are testnet-only; mainnet untested.
- **Data TTL**: IP listings and ZK proofs expire (e.g., via TTL); permanent storage not guaranteed.
- **USDC Handling**: Atomic swaps handle real USDC; users bear custody risks.
- **Dependencies**: Relies on Soroban SDK v22.0.0; upstream vulns possible.

## Out-of-Scope Items
- Attacks requiring control of user wallets or private keys.
- Theoretical attacks without practical impact.
- Previously known public issues.
- Third-party services (Stellar network, Soroban SDK, wallets).
- Denial-of-service from network congestion.
- Social engineering or phishing.

## Rewards
No formal bug bounty program yet. Responsible disclosures may receive recognition and swag/merch.

---

*Last updated: March 2026*
*Project: Atomic IP Marketplace (Soroban contracts)*
