export interface ManagedViewTransition {
  updateCallbackDone: Promise<void>;
  finished: Promise<void>;
  skipTransition: () => void;
}

export function createViewTransitionGuard() {
  let active: ManagedViewTransition | null = null;

  function track(transition: ManagedViewTransition): void {
    active = transition;
    void transition.finished
      .catch(() => undefined)
      .finally(() => {
        if (active === transition) active = null;
      });
  }

  async function finish(): Promise<void> {
    const transition = active;
    if (!transition) return;
    transition.skipTransition();
    await transition.finished.catch(() => undefined);
    if (active === transition) active = null;
  }

  function dispose(): void {
    active?.skipTransition();
    active = null;
  }

  return { track, finish, dispose };
}
