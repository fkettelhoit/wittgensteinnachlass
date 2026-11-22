// Show a compact title bar at the top of work/doc pages while scrolling up
// (once the page's own <h1> is out of view), hide it while scrolling down.
//
// When the page is opened at (or jumps to) a remark via a #fragment, show the
// bar right away and position the remark directly beneath it, so it's clear
// which work/doc the remark belongs to. The offset itself comes from
// `scroll-margin-top` in style.css.
(() => {
  const bar = document.querySelector(".sticky-title");
  const h1 = document.querySelector("main h1");
  if (!bar || !h1) return;
  let lastY = window.scrollY;
  let ticking = false;

  // Long titles wrap, so the bar's height varies; expose it for the
  // scroll-margin that keeps a jumped-to remark below the bar (style.css).
  const root = document.documentElement;
  new ResizeObserver(() =>
    root.style.setProperty("--sticky-title-height", `${bar.offsetHeight}px`),
  ).observe(bar);
  root.style.setProperty("--sticky-title-height", `${bar.offsetHeight}px`);

  const pastTitle = () => h1.getBoundingClientRect().bottom < 0;

  const update = () => {
    ticking = false;
    const y = window.scrollY;
    const delta = y - lastY;
    // Ignore jitter (e.g. momentum scrolling, address bar resizing on mobile).
    if (Math.abs(delta) < 8) return;
    lastY = y;
    bar.classList.toggle("visible", delta < 0 && pastTitle());
  };

  // The element a fragment points to, if it is part of the text (not e.g. a
  // facsimile lightbox, which is also opened via #fragment).
  const fragmentTarget = () => {
    const id = decodeURIComponent(location.hash.slice(1));
    const el = id && document.getElementById(id);
    return el && el.closest("main") && !el.closest(".lightbox") ? el : null;
  };

  const showAtTarget = () => {
    const el = fragmentTarget();
    if (!el) return;
    el.scrollIntoView({ block: "start" });
    // Record the position after the jump so it doesn't count as scrolling.
    lastY = window.scrollY;
    // Appear in place rather than sliding in.
    bar.classList.add("instant");
    bar.classList.toggle("visible", pastTitle());
    bar.getBoundingClientRect(); // apply the change before re-enabling transitions
    requestAnimationFrame(() => bar.classList.remove("instant"));
  };

  addEventListener(
    "scroll",
    () => {
      if (!ticking) {
        ticking = true;
        requestAnimationFrame(update);
      }
    },
    { passive: true },
  );

  addEventListener("hashchange", showAtTarget);
  if (fragmentTarget()) {
    showAtTarget();
    // Late-loading fonts/images can shift the layout; re-align once settled,
    // unless the reader has already started scrolling.
    addEventListener("load", () => {
      if (Math.abs(window.scrollY - lastY) < 8) showAtTarget();
    });
  }
})();
