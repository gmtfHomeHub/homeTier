// src/components/Common/Tip.tsx - Tooltip 统一封装（跨平台兼容）
//
// 桌面（hover 能力设备）：透传 @radix-ui/themes Tooltip，行为与原生一致。
// 触摸设备（hover: none）：Radix Tooltip 的悬停/focus 触发路径均不可用，
//   按内容类型自动降级：
//   - 纯文本内容 → 点按触发体弹出 toastInfo 提示
//   - 富文本内容（ReactNode）→ 长按 500ms 弹出受控 Popover 浮层
//
// 全项目禁止直接导入 @radix-ui/themes 的 Tooltip，一律使用本组件。

import {
  cloneElement,
  isValidElement,
  ReactElement,
  ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";
import { Popover, Tooltip } from "@radix-ui/themes";
import { toastInfo } from "../../utils/toast";

/** 触摸设备检测（hover: none），鼠标接入/拔出时实时响应 */
function useHoverCapable(): boolean {
  const [hoverable, setHoverable] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia("(hover: hover)").matches,
  );

  useEffect(() => {
    const mq = window.matchMedia("(hover: hover)");
    const handler = (e: MediaQueryListEvent) => setHoverable(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  return hoverable;
}

/** 组合子元素既有事件处理器与新处理器 */
function composeHandlers<T>(
  ...handlers: Array<((e: T) => void) | undefined>
): (e: T) => void {
  return (e: T) => handlers.forEach((h) => h?.(e));
}

/** 提取 ReactNode 纯文本（toast 展示用） */
function toPlainText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(toPlainText).join("");
  if (isValidElement(node)) return toPlainText(node.props.children);
  return "";
}

/** 从子元素提取 ref 与 props（cloneElement 类型辅助） */
function childProps(child: ReactElement): {
  onClick?: (e: React.MouseEvent) => void;
} {
  return (child.props ?? {}) as { onClick?: (e: React.MouseEvent) => void };
}

/* ---------------- 触摸：点按 → toast ---------------- */

function TapTip({ text, children }: { text: string; children: ReactElement }) {
  const handle = () => toastInfo(text);
  return cloneElement(children, {
    onClick: composeHandlers(childProps(children).onClick, handle),
  });
}

/* ---------------- 触摸：长按 → 受控 Popover ---------------- */

const LONG_PRESS_MS = 500;
const MOVE_CANCEL_PX = 10;

function LongPressTip({
  content,
  children,
}: {
  content: ReactNode;
  children: ReactElement;
}) {
  const [open, setOpen] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const startPos = useRef<{ x: number; y: number } | null>(null);
  const longPressedRef = useRef(false);

  const clearTimer = () => {
    if (timer.current !== undefined) {
      clearTimeout(timer.current);
      timer.current = undefined;
    }
  };

  const onPointerDown = (e: React.PointerEvent) => {
    longPressedRef.current = false;
    startPos.current = { x: e.clientX, y: e.clientY };
    clearTimer();
    timer.current = window.setTimeout(() => {
      longPressedRef.current = true;
      setOpen(true);
    }, LONG_PRESS_MS);
  };

  // 移动超阈值（滚动页面）或抬起/取消 → 取消长按
  const onPointerMove = (e: React.PointerEvent) => {
    if (!startPos.current) return;
    const dx = e.clientX - startPos.current.x;
    const dy = e.clientY - startPos.current.y;
    if (dx * dx + dy * dy > MOVE_CANCEL_PX * MOVE_CANCEL_PX) clearTimer();
  };
  const onCancel = () => clearTimer();

  // Android 长按可能唤起系统文本选择菜单；长按有效时抑制
  const onContextMenu = (e: React.MouseEvent) => {
    if (longPressedRef.current) e.preventDefault();
  };

  // 抑制长按后紧随的 click 透传给子元素动作（避免同一手势既开浮层又触发按钮）
  const onClickCapture = (e: React.MouseEvent) => {
    if (longPressedRef.current) {
      e.preventDefault();
      e.stopPropagation();
      longPressedRef.current = false;
    }
  };

  const child = children;
  const origOnClick = isValidElement(child)
    ? childProps(child).onClick
    : undefined;

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      {/* Anchor 内的触发体属于浮层边界，按压不会误关浮层 */}
      <Popover.Anchor>
        <span
          className="inline-flex"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onCancel}
          onPointerCancel={onCancel}
          onContextMenu={onContextMenu}
          onClickCapture={onClickCapture}
        >
          {cloneElement(child, {
            onClick: composeHandlers(origOnClick, () => {}),
          })}
        </span>
      </Popover.Anchor>
      <Popover.Content size="1" style={{ maxWidth: 280 }}>
        {content}
      </Popover.Content>
    </Popover.Root>
  );
}

/* ---------------- 对外接口 ---------------- */

export interface TipProps {
  /** 提示内容：纯字符串（触摸端走 toast）或任意富内容（触摸端走长按浮层） */
  content: ReactNode;
  /** 单一触发体元素 */
  children: ReactElement;
}

/**
 * 跨平台提示组件：
 * - 指针设备：Radix Tooltip 悬停显示
 * - 触摸设备：字符串内容点按弹 toast；富内容长按弹受控 Popover
 */
export function Tip({ content, children }: TipProps) {
  const hoverable = useHoverCapable();

  if (hoverable) {
    return <Tooltip content={content}>{children}</Tooltip>;
  }

  if (typeof content === "string") {
    return <TapTip text={content}>{children}</TapTip>;
  }
  return <LongPressTip content={content}>{children}</LongPressTip>;
}

export default Tip;
