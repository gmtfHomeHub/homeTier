// src/components/Common/Text.tsx — 自适应 Text 封装
//
// 包装 @radix-ui/themes Text，按全局自适应档位 L∈{1,2,3} 偏移声明的 size：
//   effective = clamp(声明size + (L-2), 1, 9)。
//   - 未声明 size（Radix 默认 "3"）：L=1/2/3 ⟹ size 2/3/4 ≈ 14/16/18px（紧凑/标准/宽松）。
//   - 声明 size：按偏移平移，保留文本相对层级。
// 其余 props 透传。全项目 Text 一律使用本组件，禁止直接从 @radix-ui/themes 导入 Text。

import { forwardRef } from "react";
import { Text as RadixText, type TextProps } from "@radix-ui/themes";
import { useSettingsStore } from "../../stores/settingsStore";

const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));

function resolveSize(size: TextProps["size"], level: number): TextProps["size"] {
  const offset = level - 2;
  if (typeof size === "string" && /^[1-9]$/.test(size)) {
    return String(clamp(Number(size) + offset, 1, 9)) as TextProps["size"];
  }
  if (size === undefined) {
    return String(clamp(3 + offset, 1, 9)) as TextProps["size"];
  }
  return size;
}

export const Text = forwardRef<HTMLSpanElement, TextProps>(
  ({ size, ...rest }, ref) => {
    const level = useSettingsStore((s) => s.adaptiveLevel);
    return <RadixText ref={ref} size={resolveSize(size, level) as TextProps["size"]} {...rest} />;
  },
);
Text.displayName = "Text";

export default Text;
