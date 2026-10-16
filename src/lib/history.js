export function formatRelativeDate(value, now = Date.now()) {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return value;
  const seconds = Math.max(0, Math.floor((now - timestamp) / 1000));
  const relative = new Intl.RelativeTimeFormat("pt-BR", { numeric: "always" });
  if (seconds < 60) return relative.format(-seconds, "second");
  if (seconds < 3600) return relative.format(-Math.floor(seconds / 60), "minute");
  if (seconds < 86400) return relative.format(-Math.floor(seconds / 3600), "hour");
  if (seconds < 604800) return relative.format(-Math.floor(seconds / 86400), "day");
  if (seconds < 2592000) return relative.format(-Math.floor(seconds / 604800), "week");
  if (seconds < 31536000) return relative.format(-Math.floor(seconds / 2592000), "month");
  return relative.format(-Math.floor(seconds / 31536000), "year");
}

export function formatAbsoluteDate(value) {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return value;
  return new Intl.DateTimeFormat("pt-BR", {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(timestamp);
}
