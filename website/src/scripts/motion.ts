// Site-wide motion: reveal on scroll, count-ups, and `data-cycle` groups that
// light one item after another on a loop. Every loop runs only while it is on
// screen. Under reduced motion everything is shown at rest.

export const still = (): boolean => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Calls `tick(step)` every `ms` milliseconds while `el` is on screen. */
export function loop(el: Element, ms: number | ((step: number) => number), tick: (step: number) => void): void {
  let step = 0;
  let timer = 0;
  let on = false;
  const run = () => {
    tick(step++);
    timer = window.setTimeout(run, typeof ms === "function" ? ms(step) : ms);
  };
  new IntersectionObserver(
    (entries) => {
      const visible = entries[entries.length - 1]?.isIntersecting ?? false;
      if (visible && !on) {
        on = true;
        run();
      } else if (!visible && on) {
        on = false;
        window.clearTimeout(timer);
      }
    },
    { rootMargin: "80px 0px" },
  ).observe(el);
}

function reveal() {
  document.querySelectorAll<HTMLElement>("[data-stagger]").forEach((group) => {
    Array.from(group.children).forEach((child, index) => {
      if (!(child instanceof HTMLElement)) return;
      if (!child.hasAttribute("data-reveal")) child.setAttribute("data-reveal", "");
      child.style.setProperty("--d", `${index * 110}ms`);
    });
  });
  const all = document.querySelectorAll<HTMLElement>("[data-reveal]");
  if (still()) {
    all.forEach((el) => el.classList.add("is-in"));
    return;
  }
  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        entry.target.classList.add("is-in");
        io.unobserve(entry.target);
      }
    },
    { threshold: 0.12, rootMargin: "0px 0px -6% 0px" },
  );
  all.forEach((el) => io.observe(el));
}

function countUp() {
  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        io.unobserve(entry.target);
        const el = entry.target as HTMLElement;
        const to = Number(el.dataset.count);
        const suffix = el.dataset.suffix ?? "";
        if (still()) {
          el.textContent = `${to}${suffix}`;
          continue;
        }
        let start = 0;
        const frame = (t: number) => {
          if (!start) start = t;
          const p = Math.min(1, (t - start) / 1500);
          el.textContent = `${Math.round(to * (1 - (1 - p) ** 4))}${suffix}`;
          if (p < 1) requestAnimationFrame(frame);
        };
        requestAnimationFrame(frame);
      }
    },
    { threshold: 0.4 },
  );
  document.querySelectorAll<HTMLElement>("[data-count]").forEach((el) => io.observe(el));
}

function cycles() {
  document.querySelectorAll<HTMLElement>("[data-cycle]").forEach((group) => {
    const items = Array.from(group.querySelectorAll<HTMLElement>("[data-cycle-item]"));
    const ms = Number(group.dataset.cycle) || 2400;
    if (!items.length) return;
    if (still()) {
      items[0]?.classList.add("is-active");
      return;
    }
    loop(group, ms, (step) => {
      items.forEach((item, index) => item.classList.toggle("is-active", index === step % items.length));
      group.style.setProperty("--cycle-ms", `${ms}ms`);
    });
  });
}

document.documentElement.classList.add("js");
reveal();
countUp();
cycles();
