import { z } from "zod";

export const settingsFormSchema = z.object({
  replaceWindowsTarget: z.boolean(),
  replaceWslTarget: z.boolean(),
  wslDistroName: z.string().optional().nullable(),
  wslUserName: z.string().optional().nullable(),
  sessionMigrationDays: z.number().min(0).max(30),
  apiKeyProviderName: z
    .string()
    .min(1)
    .regex(/^[A-Za-z0-9_-]+$/, "只能包含字母、数字、下划线或短横线"),
  sidebarCollapsed: z.boolean(),
});

export type SettingsFormValues = z.infer<typeof settingsFormSchema>;
