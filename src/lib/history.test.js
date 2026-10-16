import assert from "node:assert/strict";
import test from "node:test";
import { formatAbsoluteDate, formatRelativeDate } from "./history.js";

test("formats recent commit dates with compact relative units", () => {
  const now = Date.parse("2026-10-14T12:00:00Z");
  assert.equal(formatRelativeDate("2026-10-14T11:58:00Z", now), "há 2 minutos");
  assert.equal(formatRelativeDate("2026-10-14T09:00:00Z", now), "há 3 horas");
  assert.equal(formatRelativeDate("2026-10-12T12:00:00Z", now), "há 2 dias");
  assert.equal(formatRelativeDate("2026-09-30T12:00:00Z", now), "há 2 semanas");
});

test("returns invalid dates unchanged and formats an absolute tooltip", () => {
  assert.equal(formatRelativeDate("not-a-date"), "not-a-date");
  assert.equal(typeof formatAbsoluteDate("2026-10-14T12:00:00Z"), "string");
});
