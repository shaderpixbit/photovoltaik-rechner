<script lang="ts">
  import { page } from "$app/state";
  import { toggleMode } from "mode-watcher";
  import {
    open as openDialog,
    save as saveDialog,
  } from "@tauri-apps/plugin-dialog";
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
    DatabaseIcon,
    DownloadIcon,
    UploadIcon,
  } from "@lucide/svelte";
  import { exportBackup, importBackup, type BackupSummary } from "$lib/api";
  import { todayISO } from "$lib/utils";

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

  let backupOpen = $state(false);
  let backupBusy = $state(false);
  let backupWrap = $state<HTMLDivElement | null>(null);
  let toast = $state<{ kind: "ok" | "err"; text: string } | null>(null);

  function showToast(kind: "ok" | "err", text: string) {
    toast = { kind, text };
    setTimeout(() => (toast = null), 4000);
  }

  function summary(s: BackupSummary): string {
    return `Tage: ${s.daily} · Auszahlungen: ${s.payouts} · Ausgaben: ${s.expenses} · Anlagen: ${s.assets}`;
  }

  async function doExport() {
    backupOpen = false;
    backupBusy = true;
    try {
      const path = await saveDialog({
        defaultPath: `photovoltaik-backup-${todayISO()}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      const s = await exportBackup(path);
      showToast("ok", `Backup gespeichert. ${summary(s)}`);
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
    } finally {
      backupBusy = false;
    }
  }

  async function doImport() {
    backupOpen = false;
    if (
      !confirm(
        "Restore überschreibt ALLE bestehenden Daten dieser App (Tage, " +
          "Auszahlungen, Ausgaben, Anlagen, Verläufe, Einstellungen) mit dem " +
          "Inhalt der Backup-Datei. Fortfahren?",
      )
    ) {
      return;
    }
    backupBusy = true;
    try {
      const path = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path || typeof path !== "string") return;
      const s = await importBackup(path);
      showToast("ok", `Backup geladen. ${summary(s)}`);
      setTimeout(() => location.reload(), 800);
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
    } finally {
      backupBusy = false;
    }
  }

  $effect(() => {
    if (!backupOpen) return;
    function onDoc(e: MouseEvent) {
      if (backupWrap && !backupWrap.contains(e.target as Node)) {
        backupOpen = false;
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") backupOpen = false;
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  });
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

    <div bind:this={backupWrap} class="relative">
      <button
        type="button"
        onclick={() => (backupOpen = !backupOpen)}
        disabled={backupBusy}
        class="inline-flex h-8 items-center gap-1.5 rounded-md border border-[var(--tr-line)] px-2.5 text-sm text-[var(--tr-text-dim)] hover:bg-[var(--tr-surface2)] disabled:opacity-50"
        aria-label="Backup"
        aria-haspopup="menu"
        aria-expanded={backupOpen}
      >
        <DatabaseIcon class="size-4" />
        <span class="hidden md:inline">Backup</span>
      </button>
      {#if backupOpen}
        <div
          role="menu"
          class="absolute right-0 z-40 mt-1 w-56 overflow-hidden rounded-md border shadow-lg"
          style="background: var(--tr-surface); border-color: var(--tr-line);"
        >
          <button
            type="button"
            role="menuitem"
            onclick={doExport}
            class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--tr-surface2)]"
          >
            <DownloadIcon class="size-4 text-[var(--tr-text-faint)]" />
            Alles sichern…
          </button>
          <button
            type="button"
            role="menuitem"
            onclick={doImport}
            class="flex w-full items-center gap-2 border-t border-[var(--tr-line)] px-3 py-2 text-left text-sm hover:bg-[var(--tr-surface2)]"
          >
            <UploadIcon class="size-4 text-[var(--tr-text-faint)]" />
            Backup laden…
          </button>
        </div>
      {/if}
    </div>

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

  {#if toast}
    <div
      class="fixed bottom-6 right-6 z-50 rounded-md border px-4 py-2 text-sm shadow"
      style:background={toast.kind === "ok"
        ? "var(--tr-green-bg)"
        : "var(--tr-red-bg)"}
      style:color={toast.kind === "ok" ? "var(--tr-green-dim)" : "var(--tr-red)"}
      style:border-color={toast.kind === "ok"
        ? "var(--tr-green)"
        : "var(--tr-red)"}
    >
      {toast.text}
    </div>
  {/if}
</header>
