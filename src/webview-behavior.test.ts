import { expect, it } from "vitest";
import { suppressBrowserActions } from "./webview-behavior";

function key(target: EventTarget, name: string, modifiers = {}) {
  const event = Object.assign(new Event("keydown", { cancelable: true }), {
    key: name,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    ...modifiers,
  });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

it("blocks browser context menus including keyboard-generated menus", () => {
  const target = new EventTarget();
  const restore = suppressBrowserActions(target);
  const menu = new Event("contextmenu", { cancelable: true });
  target.dispatchEvent(menu);
  expect(menu.defaultPrevented).toBe(true);
  restore();
  const after = new Event("contextmenu", { cancelable: true });
  target.dispatchEvent(after);
  expect(after.defaultPrevented).toBe(false);
});

it("blocks reload, page save/print/open and browser navigation shortcuts", () => {
  const target = new EventTarget();
  suppressBrowserActions(target);
  for (const name of ["r", "s", "p", "o", "n", "w", "l"]) {
    expect(key(target, name, { ctrlKey: true })).toBe(true);
    expect(key(target, name.toUpperCase(), { metaKey: true, shiftKey: true })).toBe(true);
  }
  expect(key(target, "F5")).toBe(true);
  expect(key(target, "ArrowLeft", { altKey: true })).toBe(true);
  expect(key(target, "BrowserForward")).toBe(true);
});

it("preserves text editing, app search, ordinary keys and zoom", () => {
  const target = new EventTarget();
  suppressBrowserActions(target);
  for (const name of ["c", "v", "x", "a", "z", "f", "+", "-", "0"]) {
    expect(key(target, name, { ctrlKey: true })).toBe(false);
  }
  expect(key(target, "r")).toBe(false);
  expect(key(target, "ArrowLeft")).toBe(false);
  expect(key(target, "Escape")).toBe(false);
  expect(key(target, "s", { ctrlKey: true, altKey: true })).toBe(false);
});

it("restores browser shortcut handling when detached", () => {
  const target = new EventTarget();
  const restore = suppressBrowserActions(target);
  restore();
  expect(key(target, "r", { ctrlKey: true })).toBe(false);
});
