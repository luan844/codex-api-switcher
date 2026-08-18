import { create } from "zustand";

import type { AppBootstrap } from "@/types/domain";

export type AppRoute =
  | "/"
  | "/profiles"
  | "/switch"
  | "/templates"
  | "/backups"
  | "/settings"
  | "/logs";

interface AppShellState {
  bootstrap: AppBootstrap | null;
  bootstrapError: string | null;
  isBootstrapping: boolean;
  selectedProfileId: string | null;
  route: AppRoute;
  setBootstrap: (bootstrap: AppBootstrap) => void;
  setBootstrapError: (message: string | null) => void;
  setBootstrapping: (value: boolean) => void;
  setSelectedProfileId: (profileId: string | null) => void;
  setRoute: (route: AppRoute) => void;
}

export const useAppShellStore = create<AppShellState>((set) => ({
  bootstrap: null,
  bootstrapError: null,
  isBootstrapping: true,
  selectedProfileId: null,
  route: "/",
  setBootstrap: (bootstrap) =>
    set((state) => ({
      bootstrap,
      bootstrapError: null,
      selectedProfileId:
        bootstrap.profiles.some((profile) => profile.id === state.selectedProfileId)
          ? state.selectedProfileId
          : bootstrap.dashboard.activeProfileId ??
        bootstrap.profiles[0]?.id ??
        null,
    })),
  setBootstrapError: (bootstrapError) => set(() => ({ bootstrapError })),
  setBootstrapping: (isBootstrapping) => set(() => ({ isBootstrapping })),
  setSelectedProfileId: (selectedProfileId) => set(() => ({ selectedProfileId })),
  setRoute: (route) => set(() => ({ route })),
}));
