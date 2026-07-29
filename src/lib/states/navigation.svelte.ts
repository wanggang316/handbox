/**
 * Main-window route memory.
 *
 * The settings page renders inside the main window (not a separate window),
 * so the settings sidebar's "back to app" needs the main-view route that was
 * active before entering settings. The root layout calls remember() after
 * every navigation.
 */
let lastMainRoute = $state("/agent");

export const navigationState = {
  /** "Back to app" target: the most recent main-view route, default /agent. */
  get backTarget(): string {
    return lastMainRoute;
  },
  /** Record the most recent route that is neither settings nor the launch page. */
  remember(path: string): void {
    if (path === "/" || path.startsWith("/settings")) return;
    lastMainRoute = path;
  },
};
