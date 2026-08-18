import { startTransition, useEffect, useState } from "react";

import { appApi } from "@/lib/tauri-api/app";
import { useAppShellStore } from "@/store/app-shell-store";

export function useBootstrap() {
  const setBootstrap = useAppShellStore((state) => state.setBootstrap);
  const setBootstrapError = useAppShellStore((state) => state.setBootstrapError);
  const setBootstrapping = useAppShellStore((state) => state.setBootstrapping);
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const bootstrapError = useAppShellStore((state) => state.bootstrapError);
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let active = true;

    setBootstrapping(true);
    setBootstrapError(null);
    void appApi
      .loadBootstrap()
      .then((data) => {
        if (!active) {
          return;
        }
        startTransition(() => {
          setBootstrap(data);
        });
      })
      .catch((error) => {
        if (!active) {
          return;
        }
        setBootstrapError(error instanceof Error ? error.message : "启动初始化失败");
      })
      .finally(() => {
        if (!active) {
          return;
        }
        setBootstrapping(false);
      });

    return () => {
      active = false;
    };
  }, [reloadToken, setBootstrap, setBootstrapError, setBootstrapping]);

  return {
    bootstrap,
    bootstrapError,
    retryBootstrap: () => setReloadToken((value) => value + 1),
  };
}
