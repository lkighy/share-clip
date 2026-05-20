import type { ReactNode } from "react";

export function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="fluent-panel overflow-hidden">
      <div className="border-b border-slate-200/70 px-4 py-3">
        <h2 className="text-sm font-semibold text-slate-950">{title}</h2>
      </div>
      <div className="divide-y divide-slate-200/70">{children}</div>
    </section>
  );
}

export function SettingsRow({ label, children, wide = false }: { label: string; children: ReactNode; wide?: boolean }) {
  return (
    <label className={`grid gap-2 px-4 py-3 text-sm ${wide ? "sm:grid-cols-[1fr_2fr]" : "sm:grid-cols-[1fr_220px]"} sm:items-center`}>
      <span className="font-medium text-slate-700">{label}</span>
      <div className="min-w-0">{children}</div>
    </label>
  );
}
