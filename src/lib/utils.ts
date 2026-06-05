import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type WithoutChildren<T> = T extends { children?: unknown } ? Omit<T, "children"> : T;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };

/* ── German number / date formatters ─────────────────────────────────────── */

const EUR = new Intl.NumberFormat("de-DE", { style: "currency", currency: "EUR" });
const KWH = new Intl.NumberFormat("de-DE", { maximumFractionDigits: 1, minimumFractionDigits: 0 });
const PCT = new Intl.NumberFormat("de-DE", { style: "percent", maximumFractionDigits: 1 });

export function formatEUR(v: number | null | undefined): string {
  if (v == null || Number.isNaN(v)) return "—";
  return EUR.format(v);
}
export function formatKWh(v: number | null | undefined): string {
  if (v == null || Number.isNaN(v)) return "—";
  return `${KWH.format(v)} kWh`;
}
export function formatPct(v: number | null | undefined): string {
  if (v == null || Number.isNaN(v)) return "—";
  return PCT.format(v);
}

export function todayISO(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function formatDateDE(iso: string | null | undefined): string {
  if (!iso) return "—";
  const [y, m, d] = iso.slice(0, 10).split("-");
  if (!y || !m || !d) return iso;
  return `${d}.${m}.${y}`;
}

export function monthKey(iso: string): string {
  return iso.slice(0, 7);
}
export function yearKey(iso: string): string {
  return iso.slice(0, 4);
}
