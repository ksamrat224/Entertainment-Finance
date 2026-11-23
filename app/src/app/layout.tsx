import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Runway — daily savings, made playable",
  description: "Devnet-only entertainment finance prototype on Solana and MagicBlock."
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}

