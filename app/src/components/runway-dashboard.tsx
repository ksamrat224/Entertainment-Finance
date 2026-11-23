"use client";

import { useMemo, useState } from "react";
import { Activity, Check, ChevronRight, CircleAlert, ExternalLink, Gauge, ShieldCheck, Sparkles, Wallet } from "lucide-react";
import { formatMoney, truncateAddress } from "@/lib/format";
import { validateQuote } from "@/lib/market";

type Phase = "ready" | "racing" | "finalized" | "settled";
const feedId = "demo-sol-feed";

export function RunwayDashboard() {
  const [wallet, setWallet] = useState<string>();
  const [phase, setPhase] = useState<Phase>("ready");
  const [amount, setAmount] = useState("15");
  const [habit, setHabit] = useState("Packed lunch instead of ordering out");
  const [note, setNote] = useState("");
  const [checkins, setCheckins] = useState(0);
  const [score, setScore] = useState(0);
  const [allocation, setAllocation] = useState({ stability: 50, growth: 30, discovery: 20 });
  const quote = useMemo(() => validateQuote({ symbol: "SOL", price: 14235, exponent: -2, publishTime: Math.floor(Date.now() / 1000), feedId }, feedId), []);

  async function connect() {
    const provider = (window as Window & { solana?: { connect: () => Promise<{ publicKey: { toBase58: () => string } }> } }).solana;
    if (!provider) { window.alert("Install or unlock a Wallet Standard-compatible Solana wallet to continue on devnet."); return; }
    const connected = await provider.connect(); setWallet(connected.publicKey.toBase58());
  }
  function record() { const cents = Math.round(Number(amount) * 100); if (!Number.isFinite(cents) || cents <= 0) return; setCheckins((value) => Math.min(8, value + 1)); setScore((value) => value + Math.floor(cents / 100) + 10); }
  const complete = Object.values(allocation).reduce((a, b) => a + b, 0) === 100;

  return <main className="mx-auto max-w-6xl px-4 py-6 md:px-6 md:py-10">
    <header className="flex items-center justify-between gap-4 border-b border-[var(--line)] pb-5">
      <div><p className="text-xs font-semibold uppercase tracking-[.18em] text-[var(--accent)]">Runway · Devnet</p><h1 className="mt-1 text-2xl font-bold tracking-tight md:text-3xl">Make a saving move. See where it could go.</h1></div>
      <button onClick={connect} className="min-h-11 rounded-md border border-[var(--line)] bg-[var(--surface)] px-4 text-sm font-semibold hover:bg-[var(--surface-strong)]"><Wallet className="mr-2 inline size-4" aria-hidden="true" />{truncateAddress(wallet)}</button>
    </header>
    <div className="mt-5 grid gap-3 rounded-lg border border-amber-300/40 bg-amber-100/10 p-4 text-sm text-amber-50 md:grid-cols-[auto_1fr]">
      <CircleAlert className="size-5" aria-hidden="true" /><p><strong>Prototype disclosure:</strong> amounts are self-reported and illustrative. No trade, custody, payout, or investment advice is provided. Settlement writes only a devnet mock-allocation receipt.</p>
    </div>
    <section className="mt-8 grid gap-5 lg:grid-cols-[1.2fr_.8fr]">
      <div className="space-y-5">
        <div className="rise rounded-lg border border-[var(--line)] bg-[var(--surface)] p-5 md:p-6"><div className="flex items-start justify-between gap-4"><div><p className="text-sm text-[var(--muted)]">Today’s live streak race</p><h2 className="mt-1 text-xl font-bold">The 90-second runway</h2></div><span className="rounded-full bg-[var(--accent)] px-3 py-1 text-xs font-bold text-[var(--accent-ink)]">Consistency, not wealth</span></div>
          <div className="mt-6 h-3 overflow-hidden rounded-full bg-[var(--bg)]"><div className="h-full rounded-full bg-[var(--accent)] transition-all duration-300" style={{ width: `${Math.min(100, score / 4)}%` }} /></div>
          <div className="mt-3 flex items-baseline justify-between"><strong className="font-mono text-3xl tabular-nums">{score}</strong><span className="text-sm text-[var(--muted)]">{checkins}/8 check-ins · ER state</span></div>
          {phase === "ready" && <button disabled={!wallet} onClick={() => setPhase("racing")} className="mt-6 min-h-11 w-full rounded-md bg-[var(--accent)] px-4 font-bold text-[var(--accent-ink)] disabled:cursor-not-allowed disabled:opacity-45">Start a devnet run <ChevronRight className="inline size-4" aria-hidden="true" /></button>}
          {phase === "racing" && <div className="mt-6 flex items-center gap-2 text-sm text-[var(--accent)]"><Activity className="pulse size-4" aria-hidden="true" /> Live on your assigned ephemeral rollup</div>}
          {phase === "finalized" && <button onClick={() => setPhase("settled")} className="mt-6 min-h-11 w-full rounded-md bg-[var(--accent)] px-4 font-bold text-[var(--accent-ink)]">Sign devnet allocation receipt</button>}
          {phase === "settled" && <p className="mt-6 flex items-center gap-2 text-sm text-[var(--accent)]"><Check className="size-4" aria-hidden="true" /> Receipt settled on Solana. Your badge is program-owned and non-transferable.</p>}
        </div>
        <form onSubmit={(event) => { event.preventDefault(); record(); }} className="rounded-lg border border-[var(--line)] bg-[var(--surface)] p-5 md:p-6"><h2 className="text-lg font-bold">Log a saving move</h2><p className="mt-1 text-sm text-[var(--muted)]">Self-reported. This affects only your demo score.</p>
          <label className="mt-5 block text-sm font-semibold" htmlFor="habit">Habit</label><input id="habit" value={habit} onChange={(event) => setHabit(event.target.value)} className="mt-2 min-h-11 w-full rounded-md border border-[var(--line)] bg-[var(--bg)] px-3" autoComplete="off" />
          <label className="mt-4 block text-sm font-semibold" htmlFor="amount">Amount saved (USD)</label><input id="amount" type="number" inputMode="decimal" min="1" max="1000" value={amount} onChange={(event) => setAmount(event.target.value)} className="mt-2 min-h-11 w-full rounded-md border border-[var(--line)] bg-[var(--bg)] px-3" />
          <label className="mt-4 block text-sm font-semibold" htmlFor="note">Note <span className="font-normal text-[var(--muted)]">(optional, never settled)</span></label><textarea id="note" value={note} onChange={(event) => setNote(event.target.value)} maxLength={140} className="mt-2 min-h-20 w-full rounded-md border border-[var(--line)] bg-[var(--bg)] p-3" />
          <button disabled={phase !== "racing" || checkins >= 8} className="mt-5 min-h-11 w-full rounded-md border border-[var(--accent)] px-4 font-bold text-[var(--accent)] disabled:cursor-not-allowed disabled:opacity-45">Add check-in · {formatMoney(Math.round(Number(amount || 0) * 100))}</button>
          {phase === "racing" && <button type="button" onClick={() => setPhase("finalized")} className="mt-3 min-h-11 w-full rounded-md px-4 text-sm font-semibold text-[var(--muted)] underline underline-offset-4">Finish after the 60-second minimum</button>}
        </form>
      </div>
      <aside className="space-y-5"><section className="rounded-lg border border-[var(--line)] bg-[var(--surface)] p-5"><div className="flex items-center justify-between"><h2 className="font-bold">Mock portfolio lens</h2><Gauge className="size-5 text-[var(--accent)]" aria-hidden="true" /></div><p className="mt-2 text-sm text-[var(--muted)]">Educational allocation only. Nothing is bought or sold.</p><div className="mt-5 space-y-3">{(["stability", "growth", "discovery"] as const).map((key) => <label key={key} className="grid grid-cols-[1fr_auto] gap-2 text-sm capitalize">{key}<output className="font-mono tabular-nums">{allocation[key]}%</output><input className="col-span-2 accent-[var(--accent)]" type="range" min="0" max="100" value={allocation[key]} onChange={(event) => setAllocation({ ...allocation, [key]: Number(event.target.value) })} /></label>)}</div><p className={`mt-4 text-sm ${complete ? "text-[var(--accent)]" : "text-[var(--danger)]"}`}>{complete ? "Allocation is ready to settle." : "Allocation must total 100%."}</p></section>
        <section className="rounded-lg border border-[var(--line)] bg-[var(--surface)] p-5"><div className="flex items-center justify-between"><h2 className="font-bold">Market context</h2><span className="flex items-center gap-1 text-xs text-[var(--accent)]"><span className="pulse size-2 rounded-full bg-[var(--accent)]" /> fresh</span></div><div className="mt-4 flex items-baseline justify-between"><span className="text-sm text-[var(--muted)]">{quote.symbol} reference price</span><strong className="font-mono text-xl tabular-nums">${quote.displayPrice.toFixed(2)}</strong></div><p className="mt-3 text-xs leading-5 text-[var(--muted)]">Read-only Oracle display · feed identity checked · {new Date(quote.publishTime * 1000).toLocaleTimeString()} · never used for a transfer.</p></section>
        <section className="rounded-lg border border-[var(--line)] bg-[var(--surface)] p-5"><h2 className="flex items-center gap-2 font-bold"><ShieldCheck className="size-5 text-[var(--accent)]" aria-hidden="true" /> What settles</h2><ol className="mt-3 space-y-2 text-sm text-[var(--muted)]"><li>1. ER race score commits and undelegates.</li><li>2. Your wallet signs the allocation receipt.</li><li>3. Solana stores the receipt and achievement.</li></ol><a className="mt-4 inline-flex min-h-11 items-center gap-2 text-sm font-semibold text-[var(--accent)] underline underline-offset-4" href="https://explorer.solana.com/?cluster=devnet" target="_blank" rel="noreferrer">Open devnet explorer <ExternalLink className="size-4" aria-hidden="true" /></a></section>
      </aside>
    </section>
    <footer className="mt-8 flex items-center gap-2 border-t border-[var(--line)] pt-5 text-xs text-[var(--muted)]"><Sparkles className="size-4" aria-hidden="true" /> Built for MagicBlock entertainment finance · Devnet-only prototype</footer>
  </main>;
}

