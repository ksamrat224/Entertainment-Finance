export type DelegationStatus = { delegated: boolean; fqdn?: string; validator?: string };

/** Query routing state before an ER read/write; never hardcode an ER endpoint. */
export async function resolveDelegatedEndpoint(account: string): Promise<DelegationStatus> {
  const router = process.env.NEXT_PUBLIC_MAGICBLOCK_ROUTER ?? "https://devnet-router.magicblock.app/";
  const response = await fetch(router, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getDelegationStatus", params: [account] }) });
  if (!response.ok) throw new Error("Router is unavailable. Try again before sending a race action.");
  const payload = await response.json() as { result?: { delegated?: boolean; fqdn?: string; validator?: string }; error?: { message?: string } };
  if (payload.error) throw new Error(payload.error.message ?? "Router rejected the request.");
  return { delegated: Boolean(payload.result?.delegated), fqdn: payload.result?.fqdn, validator: payload.result?.validator };
}

