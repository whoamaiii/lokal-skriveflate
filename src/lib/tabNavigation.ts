export function getNextHorizontalTabId<T extends string>(
  tabs: readonly T[],
  currentTab: T,
  key: string,
): T | null {
  const currentIndex = tabs.indexOf(currentTab);

  if (currentIndex === -1) {
    return null;
  }

  switch (key) {
    case "ArrowRight":
      return tabs[(currentIndex + 1) % tabs.length];
    case "ArrowLeft":
      return tabs[(currentIndex - 1 + tabs.length) % tabs.length];
    case "Home":
      return tabs[0];
    case "End":
      return tabs[tabs.length - 1];
    default:
      return null;
  }
}
