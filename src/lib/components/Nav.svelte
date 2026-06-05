<script lang="ts">
  import { page } from "$app/state";
  import { toggleMode } from "mode-watcher";
  import {
    SunIcon,
    MoonIcon,
    GaugeIcon,
    PencilIcon,
    BanknoteIcon,
    ReceiptIcon,
    FactoryIcon,
    FileSpreadsheetIcon,
    LandmarkIcon,
    BarChart3Icon,
    SettingsIcon,
  } from "@lucide/svelte";

  const items = [
    { href: "/", label: "Dashboard", icon: GaugeIcon },
    { href: "/erfassung", label: "Tageserfassung", icon: PencilIcon },
    { href: "/auszahlungen", label: "Auszahlungen", icon: BanknoteIcon },
    { href: "/ausgaben", label: "Ausgaben", icon: ReceiptIcon },
    { href: "/anlage", label: "Anlage / AfA", icon: FactoryIcon },
    { href: "/euer", label: "EÜR", icon: FileSpreadsheetIcon },
    { href: "/ust", label: "Umsatzsteuer", icon: LandmarkIcon },
    { href: "/statistik", label: "Statistik", icon: BarChart3Icon },
    { href: "/einstellungen", label: "Einstellungen", icon: SettingsIcon },
  ];

  function isActive(href: string): boolean {
    const path = page.url.pathname;
    if (href === "/") return path === "/";
    return path === href || path.startsWith(href + "/");
  }
</script>

<header class="sticky top-0 z-30 border-b border-[var(--tr-line)] bg-[var(--tr-surface)]/85 backdrop-blur">
  <div class="mx-auto flex max-w-[1600px] items-center gap-6 px-6 py-3">
    <a href="/" class="flex items-center gap-2 font-semibold">
      <span
        class="inline-flex h-8 w-8 items-center justify-center rounded-md"
        style="background: var(--tr-sun-bg); color: var(--tr-sun);"
      >
        <SunIcon class="size-5" />
      </span>
      <span class="text-base">Photovoltaik</span>
    </a>

    <nav class="flex flex-1 flex-wrap items-center gap-1 text-sm">
      {#each items as item (item.href)}
        {@const Icon = item.icon}
        {@const active = isActive(item.href)}
        <a
          href={item.href}
          class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 transition-colors"
          class:font-medium={active}
          style:background={active ? "var(--tr-sun-bg)" : "transparent"}
          style:color={active ? "var(--tr-sun)" : "var(--tr-text-dim)"}
        >
          <Icon class="size-4" />
          {item.label}
        </a>
      {/each}
    </nav>

    <button
      type="button"
      onclick={toggleMode}
      class="inline-flex h-8 w-8 items-center justify-center rounded-md border border-[var(--tr-line)] hover:bg-[var(--tr-surface2)]"
      aria-label="Theme umschalten"
    >
      <SunIcon class="size-4 dark:hidden" />
      <MoonIcon class="hidden size-4 dark:block" />
    </button>
  </div>
</header>
