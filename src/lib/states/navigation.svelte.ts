/**
 * 主窗口路由记忆。
 *
 * 设置页在主窗口内渲染（非独立窗口），设置侧栏的「返回应用」需要知道
 * 进入设置前停在哪个主界面路由。根布局在每次导航后调用 remember()。
 */
let lastMainRoute = $state("/agent");

export const navigationState = {
  /** 「返回应用」目标：最近一次主界面路由，默认 /agent。 */
  get backTarget(): string {
    return lastMainRoute;
  },
  /** 记录最近一次非设置、非启动页的路由。 */
  remember(path: string): void {
    if (path === "/" || path.startsWith("/settings")) return;
    lastMainRoute = path;
  },
};
