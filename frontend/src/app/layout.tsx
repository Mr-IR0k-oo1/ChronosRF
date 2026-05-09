import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";

import { AppShell } from "@/components/app-shell";
import { TelemetryProvider } from "@/components/telemetry-provider";
import { fetchInitialTelemetrySnapshot } from "@/services/api";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "SpectraGuard",
  description: "Operational RF monitoring and threat detection dashboard.",
};

export default async function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const initialSnapshot = await fetchInitialTelemetrySnapshot();

  return (
    <html
      lang="en"
      data-theme="dark"
      className={`${geistSans.variable} ${geistMono.variable}`}
    >
      <body className="min-h-screen antialiased">
        <TelemetryProvider initialSnapshot={initialSnapshot}>
          <AppShell>{children}</AppShell>
        </TelemetryProvider>
      </body>
    </html>
  );
}
