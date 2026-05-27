import "./style.css";

let tickTimer: ReturnType<typeof setInterval> | null = null;

function formatClock(now: Date): { time: string; date: string } {
  return {
    time: now.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
    date: now.toLocaleDateString(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
    }),
  };
}

export function mount(container: HTMLElement): void {
  if (tickTimer !== null) {
    clearInterval(tickTimer);
    tickTimer = null;
  }

  container.classList.add("clock-panel-body");
  container.replaceChildren();

  const timeEl = document.createElement("div");
  timeEl.className = "clock-panel-time";

  const dateEl = document.createElement("div");
  dateEl.className = "clock-panel-date";

  container.append(timeEl, dateEl);

  const tick = (): void => {
    const { time, date } = formatClock(new Date());
    timeEl.textContent = time;
    dateEl.textContent = date;
  };

  tick();
  tickTimer = setInterval(tick, 1000);
}
