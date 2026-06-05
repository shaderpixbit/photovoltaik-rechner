<script lang="ts">
  import { CalendarIcon, XIcon } from "@lucide/svelte";
  import { cn, formatDateDE, todayISO } from "$lib/utils";
  import Button from "./Button.svelte";

  type Props = {
    /** ISO YYYY-MM-DD oder "" für leer. */
    value?: string;
    onchange?: (v: string) => void;
    placeholder?: string;
    disabled?: boolean;
    class?: string;
    id?: string;
  };

  let {
    value = $bindable(""),
    onchange,
    placeholder = "TT.MM.JJJJ",
    disabled = false,
    class: className,
    id,
  }: Props = $props();

  let open = $state(false);
  let draft = $state<string>(value);
  let cursor = $state(getCursor(value));
  let wrapperEl = $state<HTMLDivElement | null>(null);

  const WEEKDAYS = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
  const MONTH_FMT = new Intl.DateTimeFormat("de-DE", {
    month: "long",
    year: "numeric",
  });
  const todayIso = todayISO();

  function pad2(n: number): string {
    return String(n).padStart(2, "0");
  }
  function isoFor(y: number, m: number, d: number): string {
    return `${y}-${pad2(m)}-${pad2(d)}`;
  }
  function getCursor(iso: string): { y: number; m: number } {
    if (iso && /^\d{4}-\d{2}-\d{2}/.test(iso)) {
      const [y, m] = iso.split("-").map(Number);
      return { y, m };
    }
    const d = new Date();
    return { y: d.getFullYear(), m: d.getMonth() + 1 };
  }

  function openPopover() {
    if (disabled) return;
    draft = value;
    cursor = getCursor(value);
    open = true;
  }
  function cancel() {
    open = false;
  }
  function commit() {
    value = draft;
    onchange?.(draft);
    open = false;
  }
  function setToday() {
    draft = todayIso;
    cursor = getCursor(todayIso);
  }
  function clearDraft() {
    draft = "";
  }
  function prevMonth() {
    let { y, m } = cursor;
    m -= 1;
    if (m < 1) {
      m = 12;
      y -= 1;
    }
    cursor = { y, m };
  }
  function nextMonth() {
    let { y, m } = cursor;
    m += 1;
    if (m > 12) {
      m = 1;
      y += 1;
    }
    cursor = { y, m };
  }

  type DayCell = { iso: string; day: number; otherMonth: boolean };

  let grid = $derived.by<DayCell[]>(() => {
    const { y, m } = cursor;
    const first = new Date(y, m - 1, 1);
    const last = new Date(y, m, 0);
    // Mo-Start: getDay() Sonntag=0 → wir wollen Mo=0
    let firstDow = first.getDay() - 1;
    if (firstDow < 0) firstDow = 6;
    const cells: DayCell[] = [];
    const prevLast = new Date(y, m - 1, 0).getDate();
    const prevM = m - 1 < 1 ? 12 : m - 1;
    const prevY = m - 1 < 1 ? y - 1 : y;
    for (let i = firstDow - 1; i >= 0; i--) {
      const d = prevLast - i;
      cells.push({ iso: isoFor(prevY, prevM, d), day: d, otherMonth: true });
    }
    for (let d = 1; d <= last.getDate(); d++) {
      cells.push({ iso: isoFor(y, m, d), day: d, otherMonth: false });
    }
    const nextM = m + 1 > 12 ? 1 : m + 1;
    const nextY = m + 1 > 12 ? y + 1 : y;
    let nextDay = 1;
    while (cells.length < 42) {
      cells.push({
        iso: isoFor(nextY, nextM, nextDay),
        day: nextDay,
        otherMonth: true,
      });
      nextDay += 1;
    }
    return cells;
  });

  let monthLabel = $derived(MONTH_FMT.format(new Date(cursor.y, cursor.m - 1, 1)));

  $effect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (wrapperEl && !wrapperEl.contains(e.target as Node)) cancel();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        cancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        commit();
      }
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div bind:this={wrapperEl} class={cn("relative", className)}>
  <button
    type="button"
    {id}
    {disabled}
    class={cn(
      "flex h-9 w-full items-center justify-between gap-2 rounded-md border bg-[var(--tr-surface)] px-3 text-left text-sm",
      "border-[var(--tr-line)] hover:border-[var(--tr-line-hi)]",
      "focus:border-[var(--tr-sun)] focus:outline-none",
      "disabled:cursor-not-allowed disabled:opacity-50",
      open && "border-[var(--tr-sun)]",
    )}
    onclick={openPopover}
  >
    <span class={value ? "" : "text-[var(--tr-text-faint)]"}>
      {value ? formatDateDE(value) : placeholder}
    </span>
    <CalendarIcon class="size-4 text-[var(--tr-text-faint)]" />
  </button>

  {#if open}
    <div
      class="absolute z-40 mt-1 w-72 rounded-md border p-3 shadow-lg"
      style="background: var(--tr-surface); border-color: var(--tr-line);"
    >
      <div class="mb-2 flex items-center justify-between">
        <button
          type="button"
          class="inline-flex h-7 w-7 items-center justify-center rounded hover:bg-[var(--tr-surface2)]"
          onclick={prevMonth}
          aria-label="Vorheriger Monat"
        >
          ‹
        </button>
        <div class="text-sm font-medium capitalize">{monthLabel}</div>
        <button
          type="button"
          class="inline-flex h-7 w-7 items-center justify-center rounded hover:bg-[var(--tr-surface2)]"
          onclick={nextMonth}
          aria-label="Nächster Monat"
        >
          ›
        </button>
      </div>

      <div
        class="grid grid-cols-7 gap-y-1 text-center text-[10px] uppercase tracking-wide text-[var(--tr-text-faint)]"
      >
        {#each WEEKDAYS as w (w)}
          <div>{w}</div>
        {/each}
      </div>

      <div class="mt-1 grid grid-cols-7 gap-1">
        {#each grid as cell (cell.iso)}
          {@const isToday = cell.iso === todayIso}
          {@const isSelected = cell.iso === draft}
          <button
            type="button"
            class={cn(
              "h-8 rounded text-xs hover:bg-[var(--tr-surface2)]",
              cell.otherMonth && "text-[var(--tr-text-faint)]",
              isToday && !isSelected && "ring-1 ring-[var(--tr-sun)]",
              isSelected &&
                "bg-[var(--tr-sun)] text-black hover:bg-[var(--tr-sun)]",
            )}
            onclick={() => (draft = cell.iso)}
          >
            {cell.day}
          </button>
        {/each}
      </div>

      <div
        class="mt-3 flex items-center justify-between gap-2 border-t pt-3"
        style="border-color: var(--tr-line);"
      >
        <div class="flex gap-1">
          <Button size="sm" variant="ghost" onclick={setToday}>Heute</Button>
          <Button size="sm" variant="ghost" onclick={clearDraft}>
            <XIcon class="size-3" />Leeren
          </Button>
        </div>
        <div class="flex gap-1">
          <Button size="sm" variant="ghost" onclick={cancel}>Abbrechen</Button>
          <Button size="sm" variant="primary" onclick={commit}>OK</Button>
        </div>
      </div>
    </div>
  {/if}
</div>
