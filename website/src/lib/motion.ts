/** Every new interaction invalidates the previous timeline. No perpetual animation. */
export function timeline() {
  let generation = 0;
  return {
    cancel() {
      generation++;
    },
    async play(count: number, step: (index: number) => void, done: () => void) {
      const current = ++generation;
      for (let i = 0; i < count; i++) {
        if (current !== generation) return;
        step(i);
        if (!matchMedia("(prefers-reduced-motion: reduce)").matches)
          await new Promise((resolve) => setTimeout(resolve, 300));
      }
      if (current === generation) done();
    },
  };
}
