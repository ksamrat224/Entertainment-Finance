import { describe, expect, it } from "vitest";
import { validateQuote } from "../app/src/lib/market";

describe("market quote validation", () => {
  const now = Math.floor(Date.now() / 1000);
  it("accepts a fresh expected positive price", () => expect(validateQuote({ symbol: "SOL", price: 12345, exponent: -2, publishTime: now, feedId: "expected" }, "expected").displayPrice).toBe(123.45));
  it("rejects a wrong feed", () => expect(() => validateQuote({ symbol: "SOL", price: 1, exponent: 0, publishTime: now, feedId: "wrong" }, "expected")).toThrow("Unexpected"));
  it("rejects stale data", () => expect(() => validateQuote({ symbol: "SOL", price: 1, exponent: 0, publishTime: now - 120, feedId: "expected" }, "expected")).toThrow("stale"));
});

