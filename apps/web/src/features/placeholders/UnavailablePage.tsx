import { EmptyState } from "@wisp/ui";
import type { Section } from "../../app/router/sections";

export function UnavailablePage({ section }: { section: Section }) {
  const Icon = section.icon;
  return (
    <>
      <h1 className="mb-6 text-xl font-semibold tracking-tight">{section.label}</h1>
      <EmptyState icon={<Icon size={28} />} title="Not available yet">
        {section.unavailableNote}
      </EmptyState>
    </>
  );
}
