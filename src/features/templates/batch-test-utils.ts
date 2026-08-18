import type { BatchConnectionTestItem, CodexProfile } from "@/types/domain";

export type ProfileStatusFilter = "all" | "enabled" | "disabled";

export interface BatchTestSummary {
  autoDisabled: number;
  failure: number;
  success: number;
  warning: number;
}

export function filterProfilesByStatus(
  profiles: CodexProfile[],
  filter: ProfileStatusFilter,
) {
  if (filter === "enabled") {
    return profiles.filter((profile) => !profile.autoDisabled);
  }

  if (filter === "disabled") {
    return profiles.filter((profile) => profile.autoDisabled);
  }

  return profiles;
}

export function resolveBatchSelection(
  profiles: Pick<CodexProfile, "autoDisabled" | "id">[],
  currentSelection: string[],
  selectedProfileId: string | null,
) {
  if (!profiles.length) {
    return [];
  }

  const availableIds = new Set(
    profiles.filter((profile) => !profile.autoDisabled).map((profile) => profile.id),
  );
  const preservedSelection = currentSelection.filter((profileId) => availableIds.has(profileId));

  if (preservedSelection.length > 0) {
    return preservedSelection;
  }

  if (selectedProfileId && availableIds.has(selectedProfileId)) {
    return [selectedProfileId];
  }

  return [...availableIds];
}

export function summarizeBatchResults(results: BatchConnectionTestItem[]): BatchTestSummary {
  return results.reduce<BatchTestSummary>(
    (summary, item) => {
      if (item.status === "success") {
        summary.success += 1;
      } else if (item.status === "warning") {
        summary.warning += 1;
      } else {
        summary.failure += 1;
      }

      if (item.autoDisabled) {
        summary.autoDisabled += 1;
      }

      return summary;
    },
    { autoDisabled: 0, failure: 0, success: 0, warning: 0 },
  );
}