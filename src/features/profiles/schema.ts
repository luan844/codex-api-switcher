import { z } from "zod";

export const profileFormSchema = z.object({
  id: z.string().optional(),
  name: z.string().min(1, "请输入名称"),
  providerCategory: z.enum(["apiKey", "openAI"]),
  importConfigToml: z.boolean(),
  baseUrl: z.string().optional().nullable(),
  configTomlSourcePath: z.string().optional().nullable(),
  authMode: z.enum(["authJsonFile", "apiKey"]),
  authJsonSourcePath: z.string().optional().nullable(),
  apiKey: z.string().optional().nullable(),
  testModel: z.string().optional().nullable(),
});

export type ProfileFormValues = z.infer<typeof profileFormSchema>;
