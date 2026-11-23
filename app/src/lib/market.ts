export type MarketQuote = { symbol: string; price: number; exponent: number; publishTime: number; feedId: string };
export type ValidatedQuote = MarketQuote & { isFresh: boolean; displayPrice: number };

/** Client-side presentation guard. The program never uses this value for settlement. */
export function validateQuote(quote: MarketQuote, expectedFeedId: string, maxAgeSeconds = 90): ValidatedQuote {
  if (quote.feedId !== expectedFeedId) throw new Error("Unexpected oracle feed.");
  if (!Number.isSafeInteger(quote.price) || !Number.isInteger(quote.exponent) || quote.price <= 0) throw new Error("Invalid oracle value.");
  const age = Math.floor(Date.now() / 1000) - quote.publishTime;
  if (age < 0 || age > maxAgeSeconds) throw new Error("Market data is stale.");
  return { ...quote, isFresh: true, displayPrice: quote.price * 10 ** quote.exponent };
}

