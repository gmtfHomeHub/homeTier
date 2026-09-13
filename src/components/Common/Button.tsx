// src/components/Common/Button.tsx — 自适应 Button 封装
//
// 包装 @radix-ui/themes Button，按全局自适应档位 L∈{1,2,3}（紧凑/标准/宽松）
// 对声明的 size 做偏移：effective = clamp(声明size + (L-2), 1, 4)。
//   - 未声明 size（Radix 默认 "2"）：L=1/2/3 ⟹ size 1/2/3（满足"三档 1,2,3"）。
//   - 声明 size：按偏移平移，保留视觉层级（不把所有按钮压成同尺寸）。
// 调用点 API 与原生一致，仅覆盖 size；其余 props（variant/color/onClick/as/asChild/loading…）透传。
// 全项目 Button 一律使用本组件，禁止直接从 @radix-ui/themes 导入 Button。

import { forwardRef } from "react";
import { Button as RadixButton, type ButtonProps } from "@radix-ui/themes";
import { useSettingsStore } from "../../stores/settingsStore";

const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));

/** 仅对单数字字符串 size 做偏移；响应式对象等原样透传，未声明走 Radix 默认 "2" 推算。 */
function resolveSize(size: ButtonProps["size"], level: number): ButtonProps["size"] {
  const offset = level - 2;
  if (typeof size === "string" && /^[1-9]$/.test(size)) {
    return String(clamp(Number(size) + offset, 1, 4)) as ButtonProps["size"];
  }
  if (size === undefined) {
    return String(clamp(2 + offset, 1, 4)) as ButtonProps["size"];
  }
  return size;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ size, ...rest }, ref) => {
    const level = useSettingsStore((s) => s.adaptiveLevel);
    return <RadixButton ref={ref} size={resolveSize(size, level) as ButtonProps["size"]} {...rest} />;
  },
);
Button.displayName = "Button";

export default Button;
