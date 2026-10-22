// Suppress browser chrome actions inside the desktop workspace.
// Keep editing, search, zoom and assistive keyboard controls available.
export function suppressBrowserActions(target: EventTarget) {
  const contextMenu = (event: Event) => event.preventDefault();
  const keyDown = (event: Event) => {
    const key = event as KeyboardEvent;
    const name = key.key.toLowerCase();
    const browserShortcut =
      ((key.ctrlKey || key.metaKey) &&
        !key.altKey &&
        ["r", "s", "p", "o", "n", "w", "l"].includes(name)) ||
      name === "f5" ||
      ["browserback", "browserforward", "browserhome"].includes(name) ||
      (key.altKey && ["arrowleft", "arrowright", "home"].includes(name));
    if (browserShortcut) key.preventDefault();
  };
  target.addEventListener("contextmenu", contextMenu);
  target.addEventListener("keydown", keyDown, { capture: true });
  return () => {
    target.removeEventListener("contextmenu", contextMenu);
    target.removeEventListener("keydown", keyDown, { capture: true });
  };
}
