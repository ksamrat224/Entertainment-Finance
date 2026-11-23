export const formatMoney = (cents: number) => new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(cents / 100);
export const truncateAddress = (value?: string) => value ? `${value.slice(0, 4)}…${value.slice(-4)}` : "Not connected";

