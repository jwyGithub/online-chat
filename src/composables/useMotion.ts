import gsap from "gsap";

/** 用户是否偏好减少动态效果 */
export function reduceMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    !!window.matchMedia &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

type Target = gsap.TweenTarget;

/** 面板/元素“召唤”式入场：淡入 + 轻微上浮 + 放大。 */
export function popIn(target: Target, vars: gsap.TweenVars = {}): gsap.core.Tween | void {
  if (reduceMotion()) {
    gsap.set(target, { clearProps: "all" });
    return;
  }
  return gsap.fromTo(
    target,
    { autoAlpha: 0, y: 12, scale: 0.985 },
    {
      autoAlpha: 1,
      y: 0,
      scale: 1,
      duration: 0.42,
      ease: "power3.out",
      clearProps: "transform",
      ...vars,
    }
  );
}

/** Vue <Transition>/<TransitionGroup> 的 JS 钩子（GSAP 驱动）。 */
export function enterFade(el: Element, done: () => void, vars: gsap.TweenVars = {}) {
  if (reduceMotion()) return done();
  gsap.fromTo(
    el,
    { autoAlpha: 0, y: 10 },
    { autoAlpha: 1, y: 0, duration: 0.3, ease: "power2.out", onComplete: done, ...vars }
  );
}

export function leaveFade(el: Element, done: () => void, vars: gsap.TweenVars = {}) {
  if (reduceMotion()) return done();
  gsap.to(el, { autoAlpha: 0, y: -8, duration: 0.18, ease: "power2.in", onComplete: done, ...vars });
}

export { gsap };
